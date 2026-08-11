use crate::ids::InputDeviceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputDeviceClass {
    Keyboard,
    Pointer,
    Touchscreen,
    Tablet,
    Gamepad,
    Unknown,
}

/// What a device can produce, independent of how the backend detected it. SHER-Display
/// and Aurora branch on capabilities, never on device class strings or backend-specific
/// identifiers — a touch-capable trackpad and a touchscreen both simply
/// `supports(EventKind::Touch)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InputDeviceCapabilities {
    pub keys: bool,
    pub pointer_relative: bool,
    pub pointer_absolute: bool,
    pub scroll: bool,
    pub scroll_high_resolution: bool,
    pub touch: bool,
    pub max_touch_contacts: u32,
    pub tablet: bool,
    pub gamepad: bool,
}

impl InputDeviceCapabilities {
    pub const fn keyboard() -> Self {
        Self {
            keys: true,
            ..Self::empty()
        }
    }

    pub const fn mouse() -> Self {
        Self {
            pointer_relative: true,
            scroll: true,
            ..Self::empty()
        }
    }

    pub const fn touchscreen(max_contacts: u32) -> Self {
        Self {
            pointer_absolute: true,
            touch: true,
            max_touch_contacts: max_contacts,
            ..Self::empty()
        }
    }

    pub const fn empty() -> Self {
        Self {
            keys: false,
            pointer_relative: false,
            pointer_absolute: false,
            scroll: false,
            scroll_high_resolution: false,
            touch: false,
            max_touch_contacts: 0,
            tablet: false,
            gamepad: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}

/// Backend-specific detail that must never be interpreted by SHER-Display, Aurora, or
/// any other consumer above SHER-Input (section 3, section 10: "avoid exposing
/// backend-specific implementation details through the core API"). It exists purely
/// for diagnostics (section 28) — logging, `sher-input-monitor`, bug reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendMetadata(pub HashMap<String, String>);

impl BackendMetadata {
    pub fn insert(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }
}

/// The canonical, device-agnostic representation of an input device (section 10).
/// Everything a backend learns about `/dev/input/event7` or an HID report descriptor
/// gets translated into this shape before it leaves the backend crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputDevice {
    pub id: InputDeviceId,
    pub name: String,
    pub vendor: Option<u16>,
    pub product: Option<u16>,
    pub class: InputDeviceClass,
    pub capabilities: InputDeviceCapabilities,
    pub connection_state: ConnectionState,
    pub physical_path: Option<String>,
    pub backend_metadata: BackendMetadata,
}

impl InputDevice {
    pub fn supports(&self, predicate: impl Fn(&InputDeviceCapabilities) -> bool) -> bool {
        predicate(&self.capabilities)
    }
}
