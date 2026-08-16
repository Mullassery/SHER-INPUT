use crate::device::InputDevice;
use crate::error::Result;
use crate::gamepad::GamepadEvent;
use crate::ids::InputDeviceId;
use crate::pointer::{NormalizedPosition, PointerButton, ScrollAxis};
use crate::tablet::TabletEvent;
use crate::touch::TouchEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// What a backend reports, before [`InputService`](crate::service::InputService)
/// applies layout mapping, state tracking, sequencing, and coalescing. This is
/// intentionally more primitive than [`InputEvent`](crate::event::InputEvent) — a
/// backend knows "physical key 30 went down on device X," it does not know the active
/// keyboard layout or the device's current modifier state, both of which are
/// SHER-Input-core concerns (section 3, section 25: backend code stays Linux/HID-
/// flavored, everything layout- and state-aware lives above it).
#[derive(Debug, Clone)]
pub enum BackendEvent {
    DeviceAdded(InputDevice),
    DeviceRemoved(InputDeviceId),
    Key {
        device_id: InputDeviceId,
        physical_key: crate::keyboard::PhysicalKey,
        pressed: bool,
    },
    MotionRelative {
        device_id: InputDeviceId,
        dx: f64,
        dy: f64,
    },
    MotionAbsolute {
        device_id: InputDeviceId,
        position: NormalizedPosition,
    },
    PointerButton {
        device_id: InputDeviceId,
        button: PointerButton,
        pressed: bool,
    },
    Scroll {
        device_id: InputDeviceId,
        axis: ScrollAxis,
        delta: f64,
        high_resolution: bool,
    },
    Touch {
        device_id: InputDeviceId,
        event: TouchEvent,
    },
    Tablet {
        device_id: InputDeviceId,
        event: TabletEvent,
    },
    Gamepad {
        device_id: InputDeviceId,
        event: GamepadEvent,
    },
}

/// Handed to a backend so it can report events without holding a reference to
/// [`InputService`](crate::service::InputService) — backends depend only on
/// `sher_input_core`'s types, never on the service that consumes them (keeps the
/// dependency direction one-way, section 3).
#[derive(Clone)]
pub struct BackendEventSink(mpsc::UnboundedSender<BackendEvent>);

impl BackendEventSink {
    pub fn new(sender: mpsc::UnboundedSender<BackendEvent>) -> Self {
        Self(sender)
    }

    /// Safe to call from any thread, including a plain `std::thread` doing blocking
    /// device IO — sending never blocks and does not require a tokio runtime.
    pub fn send(&self, event: BackendEvent) {
        if self.0.send(event).is_err() {
            tracing::debug!("backend event dropped: input service no longer listening");
        }
    }
}

/// A running backend. Dropping it (or calling [`BackendHandle::stop`] explicitly)
/// signals the backend's IO loop to exit and joins it — device failure or backend
/// shutdown must never take the rest of the desktop down with it (section 27).
pub struct BackendHandle {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BackendHandle {
    pub fn new(running: Arc<AtomicBool>, thread: std::thread::JoinHandle<()>) -> Self {
        Self {
            running,
            thread: Some(thread),
        }
    }

    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for BackendHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// The contract every backend implements (section 25: "keep Linux-specific code
/// isolated" — this trait is the isolation boundary). `sher_input_linux` implements it
/// today; a future SHER-Kernel-native backend implements the same trait and Aurora
/// notices nothing.
pub trait InputBackend: Send + 'static {
    fn name(&self) -> &'static str;

    /// Spawns whatever IO the backend needs (a thread reading evdev fds, a
    /// simulated event generator, ...) and starts reporting [`BackendEvent`]s to
    /// `sink`. The backend must observe `running` and exit promptly when it flips to
    /// `false`.
    fn spawn(
        self: Box<Self>,
        sink: BackendEventSink,
        running: Arc<AtomicBool>,
    ) -> Result<std::thread::JoinHandle<()>>;
}
