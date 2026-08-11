use crate::device::{ConnectionState, InputDevice};
use crate::error::{Error, Result};
use crate::ids::InputDeviceId;
use std::collections::HashMap;
use std::sync::RwLock;

/// The central SHER-Input device registry (section 10). One instance lives inside
/// [`InputService`](crate::service::InputService); backends never hold their own copy
/// of device state — they report additions/removals and the registry is the single
/// source of truth consumers query.
#[derive(Debug, Default)]
pub struct DeviceRegistry {
    devices: RwLock<HashMap<InputDeviceId, InputDevice>>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, device: InputDevice) -> Result<()> {
        let mut devices = self.devices.write().expect("registry lock poisoned");
        if devices.contains_key(&device.id) {
            return Err(Error::DeviceAlreadyRegistered(device.id.to_string()));
        }
        devices.insert(device.id, device);
        Ok(())
    }

    pub fn remove(&self, id: InputDeviceId) -> Result<InputDevice> {
        self.devices
            .write()
            .expect("registry lock poisoned")
            .remove(&id)
            .ok_or_else(|| Error::DeviceNotFound(id.to_string()))
    }

    pub fn get(&self, id: InputDeviceId) -> Option<InputDevice> {
        self.devices.read().expect("registry lock poisoned").get(&id).cloned()
    }

    pub fn list(&self) -> Vec<InputDevice> {
        self.devices.read().expect("registry lock poisoned").values().cloned().collect()
    }

    pub fn connected(&self) -> Vec<InputDevice> {
        self.devices
            .read()
            .expect("registry lock poisoned")
            .values()
            .filter(|d| d.connection_state == ConnectionState::Connected)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.devices.read().expect("registry lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{InputDeviceCapabilities, InputDeviceClass};

    fn device(name: &str) -> InputDevice {
        InputDevice {
            id: InputDeviceId::new(),
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

    #[test]
    fn add_then_get_round_trips() {
        let registry = DeviceRegistry::new();
        let d = device("Test Keyboard");
        let id = d.id;
        registry.add(d).unwrap();

        let fetched = registry.get(id).unwrap();
        assert_eq!(fetched.name, "Test Keyboard");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn duplicate_registration_is_rejected() {
        let registry = DeviceRegistry::new();
        let d = device("Mouse");
        let dup = d.clone();
        registry.add(d).unwrap();

        assert!(matches!(registry.add(dup), Err(Error::DeviceAlreadyRegistered(_))));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn removing_unknown_device_errors() {
        let registry = DeviceRegistry::new();
        assert!(matches!(registry.remove(InputDeviceId::new()), Err(Error::DeviceNotFound(_))));
    }

    #[test]
    fn remove_forgets_the_device() {
        let registry = DeviceRegistry::new();
        let d = device("Touchpad");
        let id = d.id;
        registry.add(d).unwrap();

        registry.remove(id).unwrap();
        assert!(registry.get(id).is_none());
        assert!(registry.is_empty());
    }
}
