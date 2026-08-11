use crate::backend::{BackendEvent, BackendEventSink, BackendHandle, InputBackend};
use crate::capture::{CaptureGuard, CaptureKind, CaptureRegistry, PointerGrabMode};
use crate::coalesce::Coalescer;
use crate::config::InputConfig;
use crate::error::{Error, Result};
use crate::event::{InputEvent, InputEventPayload};
use crate::ids::{InputDeviceId, SequenceCounter, Timestamp};
use crate::keyboard::{KeyAction, KeyboardEvent, KeyboardLayout, KeyboardStateTable, Modifiers, UsQwertyLayout};
use crate::pointer::{PointerEvent, PointerStateTable};
use crate::registry::DeviceRegistry;
use crate::source::{InputSource, SyntheticInputGrant};
use crate::touch::TouchStateTable;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{broadcast, mpsc};

/// The SHER-Input orchestrator (section 11, 25). Owns the device registry, per-device
/// keyboard/pointer/touch state, sequencing, coalescing, and capture bookkeeping;
/// backends only ever talk to it through [`BackendEventSink`] and never see each
/// other. SHER-Display (and any other consumer, including `sher-input-monitor`) talks
/// to it only through [`InputService::subscribe`] and the registry/capture accessors —
/// this struct *is* the SHER-Kernel/backend ↔ SHER-Input ↔ SHER-Display boundary made
/// concrete (section 33).
pub struct InputService {
    registry: DeviceRegistry,
    sequence: SequenceCounter,
    config: RwLock<InputConfig>,
    layout: RwLock<Box<dyn KeyboardLayout>>,
    keyboard_state: Mutex<KeyboardStateTable>,
    pointer_state: Mutex<PointerStateTable>,
    touch_state: Mutex<TouchStateTable>,
    coalescer: Mutex<Coalescer>,
    captures: Arc<CaptureRegistry>,
    events_tx: broadcast::Sender<InputEvent>,
    backend_events_tx: mpsc::UnboundedSender<BackendEvent>,
}

impl InputService {
    /// Must be called from within a running tokio runtime — it spawns the task that
    /// pumps backend events into normalization (section 13: event-driven, not polled,
    /// so this task is the only thing standing between hardware and a subscriber).
    pub fn new(config: InputConfig) -> Arc<Self> {
        let (backend_events_tx, mut backend_events_rx) = mpsc::unbounded_channel();
        let (events_tx, _) = broadcast::channel(4096);

        let service = Arc::new(Self {
            registry: DeviceRegistry::new(),
            sequence: SequenceCounter::new(),
            config: RwLock::new(config),
            layout: RwLock::new(Box::new(UsQwertyLayout)),
            keyboard_state: Mutex::new(KeyboardStateTable::new()),
            pointer_state: Mutex::new(PointerStateTable::new()),
            touch_state: Mutex::new(TouchStateTable::new()),
            coalescer: Mutex::new(Coalescer::new()),
            captures: CaptureRegistry::new(),
            events_tx,
            backend_events_tx,
        });

        // Weak, not Arc::clone: a strong self-reference here would mean the pump task
        // keeps `service` alive forever, which keeps `backend_events_tx` (a field of
        // `service`) alive, which keeps the channel open, which keeps this very task
        // alive — a reference cycle that leaks the service permanently. Once every
        // external `Arc<InputService>` is dropped, `service` (and its `backend_events_tx`
        // field) drops, the channel closes, `recv()` returns `None`, and the loop ends.
        let pump_target = Arc::downgrade(&service);
        tokio::spawn(async move {
            while let Some(event) = backend_events_rx.recv().await {
                if let Some(service) = pump_target.upgrade() {
                    service.ingest(event);
                }
            }
        });

        // Section 14's coalescing is only safe because something eventually flushes
        // pending motion/scroll even if no consumer calls `flush_coalesced()` (a lone
        // mouse nudge must still arrive, not wait forever for a second event on the
        // same device). SHER-Display is expected to call `flush_coalesced()` once per
        // frame for tighter latency; this ticker is the bound for everyone else.
        let ticker_target = Arc::downgrade(&service);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(8));
            loop {
                interval.tick().await;
                match ticker_target.upgrade() {
                    Some(service) => service.flush_coalesced(),
                    None => break,
                }
            }
        });

        service
    }

    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    pub fn captures(&self) -> &Arc<CaptureRegistry> {
        &self.captures
    }

    pub fn config(&self) -> InputConfig {
        self.config.read().expect("config lock poisoned").clone()
    }

    pub fn set_config(&self, config: InputConfig) {
        *self.config.write().expect("config lock poisoned") = config;
    }

    pub fn set_layout(&self, layout: Box<dyn KeyboardLayout>) {
        *self.layout.write().expect("layout lock poisoned") = layout;
    }

    /// The canonical, ordered, coalesced event stream (section 11-14). Every consumer
    /// — SHER-Display, `sher-input-monitor`, tests — gets its own receiver; none of
    /// them can starve or block another (a slow diagnostics tool cannot add latency to
    /// SHER-Display's copy, section 13).
    pub fn subscribe(&self) -> broadcast::Receiver<InputEvent> {
        self.events_tx.subscribe()
    }

    /// A sink a backend can push [`BackendEvent`]s into. Handed to
    /// [`InputBackend::spawn`] by [`InputService::start_backend`] — exposed directly
    /// too, for backends/tests that want to drive ingestion without a full
    /// `InputBackend` implementation.
    pub fn sink(&self) -> BackendEventSink {
        BackendEventSink::new(self.backend_events_tx.clone())
    }

    /// Starts a backend's IO loop and returns a handle that stops it on drop (section
    /// 27: a backend crash or explicit stop must not take SHER-Input itself down).
    pub fn start_backend(&self, backend: Box<dyn InputBackend>) -> Result<BackendHandle> {
        let running = Arc::new(AtomicBool::new(true));
        let sink = self.sink();
        let thread = backend.spawn(sink, Arc::clone(&running))?;
        Ok(BackendHandle::new(running, thread))
    }

    /// SHER-Display's entry point for section 16 grabs. `pointer_mode` is ignored for
    /// non-pointer captures.
    pub fn request_capture(
        self: &Arc<Self>,
        kind: CaptureKind,
        owner: impl Into<String>,
        reason: impl Into<String>,
        pointer_mode: Option<PointerGrabMode>,
    ) -> Result<CaptureGuard> {
        self.captures.acquire(kind, owner, reason, pointer_mode)
    }

    /// Section 22-24: synthetic input is only accepted with a grant scoped to the
    /// event kind being submitted, and is always tagged
    /// [`InputSource::Synthetic`] so no consumer can mistake it for a physical key
    /// press — including an AI agent's own view of what it just did.
    pub fn submit_synthetic(&self, grant: &SyntheticInputGrant, event: BackendEvent) -> Result<()> {
        let allowed = match &event {
            BackendEvent::Key { .. } => grant.allows_keyboard,
            BackendEvent::MotionRelative { .. }
            | BackendEvent::MotionAbsolute { .. }
            | BackendEvent::PointerButton { .. }
            | BackendEvent::Scroll { .. } => grant.allows_pointer,
            BackendEvent::Touch { .. } => grant.allows_touch,
            BackendEvent::DeviceAdded(_) | BackendEvent::DeviceRemoved(_) => true,
            BackendEvent::Tablet { .. } | BackendEvent::Gamepad { .. } => false,
        };
        if !allowed {
            return Err(Error::SyntheticInputDenied(format!(
                "grant for {:?} does not cover this event kind",
                grant.origin
            )));
        }
        self.ingest_with_source(event, InputSource::Synthetic(grant.origin));
        Ok(())
    }

    /// Forces delivery of any coalesced-but-not-yet-emitted motion/scroll samples
    /// (section 14) — call once per compositor frame so coalescing never adds more
    /// than one frame of latency.
    pub fn flush_coalesced(&self) {
        let ready = self.coalescer.lock().expect("coalescer lock poisoned").drain_all();
        for event in ready {
            let _ = self.events_tx.send(event);
        }
    }

    fn ingest(&self, event: BackendEvent) {
        self.ingest_with_source(event, InputSource::Physical);
    }

    fn ingest_with_source(&self, event: BackendEvent, source: InputSource) {
        match event {
            BackendEvent::DeviceAdded(device) => {
                let device_id = device.id;
                if let Err(err) = self.registry.add(device.clone()) {
                    tracing::warn!("{err}");
                    return;
                }
                self.emit(device_id, source, Modifiers::default(), InputEventPayload::DeviceAdded(device));
            }
            BackendEvent::DeviceRemoved(device_id) => {
                if self.registry.remove(device_id).is_err() {
                    tracing::warn!("device removed but was not registered: {device_id}");
                }
                self.keyboard_state.lock().expect("keyboard lock poisoned").remove(device_id);
                self.pointer_state.lock().expect("pointer lock poisoned").remove(device_id);
                self.touch_state.lock().expect("touch lock poisoned").remove(device_id);
                self.coalescer.lock().expect("coalescer lock poisoned").drain_device(device_id);
                self.emit(device_id, source, Modifiers::default(), InputEventPayload::DeviceRemoved(device_id));
            }
            BackendEvent::Key { device_id, physical_key, pressed } => {
                let (modifiers, is_repeat) = {
                    let mut table = self.keyboard_state.lock().expect("keyboard lock poisoned");
                    let state = table.state_mut(device_id);
                    let was_pressed = state.is_pressed(physical_key);
                    if pressed {
                        state.note_down(physical_key);
                    } else {
                        state.note_up(physical_key);
                    }
                    (state.modifiers(), pressed && was_pressed)
                };
                let (logical, text) = {
                    let layout = self.layout.read().expect("layout lock poisoned");
                    let logical = layout.map(physical_key, modifiers);
                    let text = if pressed { layout.text_for(logical, modifiers) } else { None };
                    (logical, text)
                };
                let action = if !pressed {
                    KeyAction::Up
                } else if is_repeat {
                    KeyAction::Repeat
                } else {
                    KeyAction::Down
                };
                let payload = InputEventPayload::Keyboard(KeyboardEvent {
                    physical_key,
                    logical_key: logical,
                    text,
                    action,
                });
                self.emit(device_id, source, modifiers, payload);
            }
            BackendEvent::MotionRelative { device_id, dx, dy } => {
                self.pointer_state
                    .lock()
                    .expect("pointer lock poisoned")
                    .state_mut(device_id)
                    .apply_relative(dx, dy);
                let modifiers = self.aggregate_modifiers();
                self.emit(device_id, source, modifiers, InputEventPayload::Pointer(PointerEvent::MotionRelative { dx, dy }));
            }
            BackendEvent::MotionAbsolute { device_id, position } => {
                self.pointer_state
                    .lock()
                    .expect("pointer lock poisoned")
                    .state_mut(device_id)
                    .apply_absolute(position);
                let modifiers = self.aggregate_modifiers();
                self.emit(
                    device_id,
                    source,
                    modifiers,
                    InputEventPayload::Pointer(PointerEvent::MotionAbsolute { position }),
                );
            }
            BackendEvent::PointerButton { device_id, button, pressed } => {
                self.pointer_state
                    .lock()
                    .expect("pointer lock poisoned")
                    .state_mut(device_id)
                    .note_button(button, pressed);
                let modifiers = self.aggregate_modifiers();
                self.emit(device_id, source, modifiers, InputEventPayload::Pointer(PointerEvent::Button { button, pressed }));
            }
            BackendEvent::Scroll { device_id, axis, delta, high_resolution } => {
                let modifiers = self.aggregate_modifiers();
                self.emit(
                    device_id,
                    source,
                    modifiers,
                    InputEventPayload::Pointer(PointerEvent::Scroll { axis, delta, high_resolution }),
                );
            }
            BackendEvent::Touch { device_id, event } => {
                self.touch_state
                    .lock()
                    .expect("touch lock poisoned")
                    .state_mut(device_id)
                    .apply(&event);
                let modifiers = self.aggregate_modifiers();
                self.emit(device_id, source, modifiers, InputEventPayload::Touch(event));
            }
            BackendEvent::Tablet { device_id, event } => {
                let modifiers = self.aggregate_modifiers();
                self.emit(device_id, source, modifiers, InputEventPayload::Tablet(event));
            }
            BackendEvent::Gamepad { device_id, event } => {
                self.emit(device_id, source, Modifiers::default(), InputEventPayload::Gamepad(event));
            }
        }
    }

    fn aggregate_modifiers(&self) -> Modifiers {
        self.keyboard_state.lock().expect("keyboard lock poisoned").aggregate()
    }

    fn emit(&self, device_id: InputDeviceId, source: InputSource, modifiers: Modifiers, payload: InputEventPayload) {
        let event = InputEvent {
            timestamp: Timestamp::now(),
            sequence: self.sequence.next(),
            device_id,
            source,
            modifiers,
            payload,
        };
        let ready = self.coalescer.lock().expect("coalescer lock poisoned").offer(event);
        for event in ready {
            // No subscribers is a normal state (e.g. before SHER-Display attaches);
            // broadcast::Sender::send only errors when there are zero receivers.
            let _ = self.events_tx.send(event);
        }
    }
}
