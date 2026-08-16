use sher_input_core::{
    BackendMetadata, ConnectionState, InputDevice, InputDeviceCapabilities, InputDeviceClass,
    InputDeviceId,
};

pub fn keyboard_device(name: impl Into<String>) -> InputDevice {
    InputDevice {
        id: InputDeviceId::new(),
        name: name.into(),
        vendor: None,
        product: None,
        class: InputDeviceClass::Keyboard,
        capabilities: InputDeviceCapabilities::keyboard(),
        connection_state: ConnectionState::Connected,
        physical_path: None,
        backend_metadata: BackendMetadata::default().insert("backend", "simulated"),
    }
}

pub fn mouse_device(name: impl Into<String>) -> InputDevice {
    InputDevice {
        id: InputDeviceId::new(),
        name: name.into(),
        vendor: None,
        product: None,
        class: InputDeviceClass::Pointer,
        capabilities: InputDeviceCapabilities::mouse(),
        connection_state: ConnectionState::Connected,
        physical_path: None,
        backend_metadata: BackendMetadata::default().insert("backend", "simulated"),
    }
}

pub fn touchscreen_device(name: impl Into<String>, max_contacts: u32) -> InputDevice {
    InputDevice {
        id: InputDeviceId::new(),
        name: name.into(),
        vendor: None,
        product: None,
        class: InputDeviceClass::Touchscreen,
        capabilities: InputDeviceCapabilities::touchscreen(max_contacts),
        connection_state: ConnectionState::Connected,
        physical_path: None,
        backend_metadata: BackendMetadata::default().insert("backend", "simulated"),
    }
}
