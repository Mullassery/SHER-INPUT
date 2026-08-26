//! Proves section 27's crash-isolation claim ("a backend thread panicking ...
//! does not take InputService down") is actually true, not just documented, and
//! that a crashed backend can be hot-restarted: `BackendHandle::is_finished`/
//! `stop_and_check_panicked` let a caller detect the crash, and starting a fresh
//! backend through the same `InputService` resumes real event delivery.

use sher_input_core::{
    BackendEvent, BackendEventSink, ConnectionState, Error, InputBackend, InputConfig, InputDevice,
    InputDeviceCapabilities, InputDeviceClass, InputDeviceId, InputEventPayload, InputService,
    Result,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn keyboard(id: InputDeviceId, name: &str) -> InputDevice {
    InputDevice {
        id,
        name: name.to_string(),
        vendor: None,
        product: None,
        class: InputDeviceClass::Keyboard,
        capabilities: InputDeviceCapabilities::keyboard(),
        connection_state: ConnectionState::Connected,
        physical_path: None,
        backend_metadata: Default::default(),
    }
}

/// A real (if fake) `InputBackend` whose IO thread panics immediately —
/// the simplest genuine stand-in for a driver crash (a malformed report, a
/// bug in device-specific parsing, ...).
struct PanickingBackend;

impl InputBackend for PanickingBackend {
    fn name(&self) -> &'static str {
        "test::PanickingBackend"
    }

    fn spawn(
        self: Box<Self>,
        _sink: BackendEventSink,
        _running: Arc<AtomicBool>,
    ) -> Result<std::thread::JoinHandle<()>> {
        std::thread::Builder::new()
            .name("panicking-backend".to_string())
            .spawn(|| panic!("simulated backend crash"))
            .map_err(|e| Error::Backend(e.to_string()))
    }
}

/// A real `InputBackend` that reports one device-added event, proving it's a
/// genuinely working replacement, then idles — used as the fresh instance a
/// hot-restart spawns after `PanickingBackend` crashes.
struct OneEventBackend {
    device_id: InputDeviceId,
}

impl InputBackend for OneEventBackend {
    fn name(&self) -> &'static str {
        "test::OneEventBackend"
    }

    fn spawn(
        self: Box<Self>,
        sink: BackendEventSink,
        running: Arc<AtomicBool>,
    ) -> Result<std::thread::JoinHandle<()>> {
        let device_id = self.device_id;
        std::thread::Builder::new()
            .name("one-event-backend".to_string())
            .spawn(move || {
                sink.send(BackendEvent::DeviceAdded(keyboard(
                    device_id,
                    "Restarted Keyboard",
                )));
                while running.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(20));
                }
            })
            .map_err(|e| Error::Backend(e.to_string()))
    }
}

fn wait_until_finished(handle: &sher_input_core::BackendHandle, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() {
        assert!(
            Instant::now() < deadline,
            "backend did not finish within {timeout:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[tokio::test]
async fn a_panicking_backend_is_detected_without_blocking() {
    let service = InputService::new(InputConfig::default());
    let handle = service.start_backend(Box::new(PanickingBackend)).unwrap();

    wait_until_finished(&handle, Duration::from_secs(2));
    assert!(handle.stop_and_check_panicked());
}

#[tokio::test]
async fn a_backend_panic_does_not_take_input_service_down() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();

    let handle = service.start_backend(Box::new(PanickingBackend)).unwrap();
    wait_until_finished(&handle, Duration::from_secs(2));
    handle.stop_and_check_panicked();

    // The service is still alive and functional after the crash: a direct
    // sink push (as any live backend would do) is still delivered.
    let sink = service.sink();
    let id = InputDeviceId::new();
    sink.send(BackendEvent::DeviceAdded(keyboard(id, "Still Works")));

    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("InputService should still be delivering events after a backend crash")
        .unwrap();
    assert!(matches!(event.payload, InputEventPayload::DeviceAdded(_)));
    assert_eq!(service.registry().len(), 1);
}

#[tokio::test]
async fn a_crashed_backend_can_be_hot_restarted_with_a_fresh_instance() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();

    let crashed = service.start_backend(Box::new(PanickingBackend)).unwrap();
    wait_until_finished(&crashed, Duration::from_secs(2));
    assert!(
        crashed.stop_and_check_panicked(),
        "precondition: the backend must have actually crashed"
    );

    // Hot-restart: same InputService, a fresh backend instance -- exactly
    // what a caller's restart policy would do on detecting the crash above.
    let device_id = InputDeviceId::new();
    let restarted = service
        .start_backend(Box::new(OneEventBackend { device_id }))
        .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("the restarted backend should deliver real events")
        .unwrap();
    assert!(matches!(event.payload, InputEventPayload::DeviceAdded(_)));
    assert_eq!(service.registry().len(), 1);
    assert!(service.registry().get(device_id).is_some());

    // Clean shutdown of the healthy backend must not be misreported as a
    // crash -- `stop_and_check_panicked` distinguishes an intentional stop
    // from a panic.
    assert!(!restarted.stop_and_check_panicked());
}
