use sher_input_core::{
    BackendEvent, CaptureKind, ConnectionState, InputConfig, InputDevice, InputDeviceCapabilities, InputDeviceClass,
    InputDeviceId, InputEventPayload, InputService, KeyAction, PhysicalKey, PointerButton, SyntheticInputGrant,
    SyntheticOrigin,
};

fn keyboard(id: InputDeviceId) -> InputDevice {
    InputDevice {
        id,
        name: "Test Keyboard".to_string(),
        vendor: None,
        product: None,
        class: InputDeviceClass::Keyboard,
        capabilities: InputDeviceCapabilities::keyboard(),
        connection_state: ConnectionState::Connected,
        physical_path: None,
        backend_metadata: Default::default(),
    }
}

fn mouse(id: InputDeviceId) -> InputDevice {
    InputDevice {
        id,
        name: "Test Mouse".to_string(),
        vendor: None,
        product: None,
        class: InputDeviceClass::Pointer,
        capabilities: InputDeviceCapabilities::mouse(),
        connection_state: ConnectionState::Connected,
        physical_path: None,
        backend_metadata: Default::default(),
    }
}

#[tokio::test]
async fn hotplug_add_registers_device_and_emits_device_added() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let sink = service.sink();
    let id = InputDeviceId::new();

    sink.send(BackendEvent::DeviceAdded(keyboard(id)));

    let event = events.recv().await.unwrap();
    assert!(matches!(event.payload, InputEventPayload::DeviceAdded(_)));
    assert_eq!(service.registry().len(), 1);
}

#[tokio::test]
async fn key_press_then_release_produces_down_then_up_in_order() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let sink = service.sink();
    let id = InputDeviceId::new();

    sink.send(BackendEvent::DeviceAdded(keyboard(id)));
    sink.send(BackendEvent::Key { device_id: id, physical_key: PhysicalKey::KeyA, pressed: true });
    sink.send(BackendEvent::Key { device_id: id, physical_key: PhysicalKey::KeyA, pressed: false });

    let added = events.recv().await.unwrap();
    assert!(matches!(added.payload, InputEventPayload::DeviceAdded(_)));

    let down = events.recv().await.unwrap();
    let up = events.recv().await.unwrap();

    match down.payload {
        InputEventPayload::Keyboard(k) => assert_eq!(k.action, KeyAction::Down),
        other => panic!("expected keyboard down, got {other:?}"),
    }
    match up.payload {
        InputEventPayload::Keyboard(k) => assert_eq!(k.action, KeyAction::Up),
        other => panic!("expected keyboard up, got {other:?}"),
    }
    // Ordering must hold even though both events landed in the same nanosecond on a
    // fast machine — sequence numbers are the authority (section 12).
    assert!(down.sequence.0 < up.sequence.0);
}

#[tokio::test]
async fn removing_a_device_clears_its_state_and_emits_device_removed() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let sink = service.sink();
    let id = InputDeviceId::new();

    sink.send(BackendEvent::DeviceAdded(keyboard(id)));
    sink.send(BackendEvent::Key { device_id: id, physical_key: PhysicalKey::ControlLeft, pressed: true });
    sink.send(BackendEvent::DeviceRemoved(id));

    let _added = events.recv().await.unwrap();
    let _key_down = events.recv().await.unwrap();
    let removed = events.recv().await.unwrap();

    assert!(matches!(removed.payload, InputEventPayload::DeviceRemoved(removed_id) if removed_id == id));
    assert!(service.registry().get(id).is_none());
    assert_eq!(service.registry().len(), 0);
}

#[tokio::test]
async fn events_from_two_devices_are_each_internally_ordered() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let sink = service.sink();
    let kb = InputDeviceId::new();
    let ms = InputDeviceId::new();

    sink.send(BackendEvent::DeviceAdded(keyboard(kb)));
    sink.send(BackendEvent::DeviceAdded(mouse(ms)));
    sink.send(BackendEvent::Key { device_id: kb, physical_key: PhysicalKey::KeyA, pressed: true });
    sink.send(BackendEvent::PointerButton { device_id: ms, button: PointerButton::Left, pressed: true });

    let mut sequences = Vec::new();
    for _ in 0..4 {
        sequences.push(events.recv().await.unwrap().sequence.0);
    }
    let mut sorted = sequences.clone();
    sorted.sort();
    assert_eq!(sequences, sorted, "sequence numbers must be monotonically increasing across devices");
}

#[tokio::test]
async fn pointer_capture_is_exclusive_and_revocable() {
    let service = InputService::new(InputConfig::default());

    let guard = service
        .request_capture(CaptureKind::Pointer, "aurora.drag", "window drag", None)
        .expect("first capture should succeed");
    assert!(service.captures().is_captured(CaptureKind::Pointer));

    let denied = service.request_capture(CaptureKind::Pointer, "other", "conflicting grab", None);
    assert!(denied.is_err());

    drop(guard);
    assert!(!service.captures().is_captured(CaptureKind::Pointer));

    let reacquired = service.request_capture(CaptureKind::Pointer, "aurora.drag", "second drag", None);
    assert!(reacquired.is_ok());
}

#[tokio::test]
async fn synthetic_input_is_tagged_and_rejected_without_a_grant() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let id = InputDeviceId::new();

    let keyboard_only = SyntheticInputGrant::new(SyntheticOrigin::AccessibilityTool).with_keyboard();

    let rejected = service.submit_synthetic(
        &keyboard_only,
        BackendEvent::PointerButton { device_id: id, button: PointerButton::Left, pressed: true },
    );
    assert!(rejected.is_err(), "grant without pointer permission must not be able to move the pointer");

    service
        .submit_synthetic(&keyboard_only, BackendEvent::Key { device_id: id, physical_key: PhysicalKey::KeyA, pressed: true })
        .expect("grant with keyboard permission should succeed");

    let event = events.recv().await.unwrap();
    assert!(matches!(event.source, sher_input_core::InputSource::Synthetic(SyntheticOrigin::AccessibilityTool)));
}
