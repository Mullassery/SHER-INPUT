use crate::devices::{keyboard_device, mouse_device, touchscreen_device};
use sher_input_core::{
    BackendEvent, BackendEventSink, ContactId, InputDeviceId, NormalizedPosition, PhysicalKey,
    PointerButton, ScrollAxis, TouchEvent, TouchPhase,
};

/// Drives an [`InputService`](sher_input_core::InputService) as if a real backend were
/// producing hardware activity, without needing Linux, evdev, or physical devices.
/// This is the primary way SHER-Input's own tests exercise the service, and it is
/// meant to be reused by anything downstream (SHER-Display's tests, Aurora's tests,
/// CI) that needs deterministic input without hardware — swap it in for
/// `sher_input_linux` and nothing above `InputService` can tell the difference.
pub struct SimulatedController {
    sink: BackendEventSink,
}

impl SimulatedController {
    pub fn new(sink: BackendEventSink) -> Self {
        Self { sink }
    }

    pub fn add_keyboard(&self, name: impl Into<String>) -> InputDeviceId {
        let device = keyboard_device(name);
        let id = device.id;
        self.sink.send(BackendEvent::DeviceAdded(device));
        id
    }

    pub fn add_mouse(&self, name: impl Into<String>) -> InputDeviceId {
        let device = mouse_device(name);
        let id = device.id;
        self.sink.send(BackendEvent::DeviceAdded(device));
        id
    }

    pub fn add_touchscreen(&self, name: impl Into<String>, max_contacts: u32) -> InputDeviceId {
        let device = touchscreen_device(name, max_contacts);
        let id = device.id;
        self.sink.send(BackendEvent::DeviceAdded(device));
        id
    }

    pub fn unplug(&self, device_id: InputDeviceId) {
        self.sink.send(BackendEvent::DeviceRemoved(device_id));
    }

    pub fn press_key(&self, device_id: InputDeviceId, key: PhysicalKey) {
        self.sink.send(BackendEvent::Key {
            device_id,
            physical_key: key,
            pressed: true,
        });
    }

    pub fn release_key(&self, device_id: InputDeviceId, key: PhysicalKey) {
        self.sink.send(BackendEvent::Key {
            device_id,
            physical_key: key,
            pressed: false,
        });
    }

    pub fn tap_key(&self, device_id: InputDeviceId, key: PhysicalKey) {
        self.press_key(device_id, key);
        self.release_key(device_id, key);
    }

    pub fn move_relative(&self, device_id: InputDeviceId, dx: f64, dy: f64) {
        self.sink
            .send(BackendEvent::MotionRelative { device_id, dx, dy });
    }

    pub fn move_absolute(&self, device_id: InputDeviceId, x: f64, y: f64) {
        self.sink.send(BackendEvent::MotionAbsolute {
            device_id,
            position: NormalizedPosition { x, y },
        });
    }

    pub fn press_button(&self, device_id: InputDeviceId, button: PointerButton) {
        self.sink.send(BackendEvent::PointerButton {
            device_id,
            button,
            pressed: true,
        });
    }

    pub fn release_button(&self, device_id: InputDeviceId, button: PointerButton) {
        self.sink.send(BackendEvent::PointerButton {
            device_id,
            button,
            pressed: false,
        });
    }

    pub fn click(&self, device_id: InputDeviceId, button: PointerButton) {
        self.press_button(device_id, button);
        self.release_button(device_id, button);
    }

    pub fn scroll(&self, device_id: InputDeviceId, axis: ScrollAxis, delta: f64) {
        self.sink.send(BackendEvent::Scroll {
            device_id,
            axis,
            delta,
            high_resolution: false,
        });
    }

    pub fn touch_down(&self, device_id: InputDeviceId, contact: ContactId, x: f64, y: f64) {
        self.sink.send(BackendEvent::Touch {
            device_id,
            event: TouchEvent {
                contact_id: contact,
                phase: TouchPhase::Down,
                position: NormalizedPosition { x, y },
                pressure: None,
                size: None,
            },
        });
    }

    pub fn touch_move(&self, device_id: InputDeviceId, contact: ContactId, x: f64, y: f64) {
        self.sink.send(BackendEvent::Touch {
            device_id,
            event: TouchEvent {
                contact_id: contact,
                phase: TouchPhase::Move,
                position: NormalizedPosition { x, y },
                pressure: None,
                size: None,
            },
        });
    }

    pub fn touch_up(&self, device_id: InputDeviceId, contact: ContactId) {
        self.sink.send(BackendEvent::Touch {
            device_id,
            event: TouchEvent {
                contact_id: contact,
                phase: TouchPhase::Up,
                position: NormalizedPosition { x: 0.0, y: 0.0 },
                pressure: None,
                size: None,
            },
        });
    }
}
