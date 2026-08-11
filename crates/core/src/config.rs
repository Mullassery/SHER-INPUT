use serde::{Deserialize, Serialize};

/// Section 20: configuration is deliberately its own layer, held by
/// [`InputService`](crate::service::InputService) and applied when normalizing raw
/// backend events — a backend never reads config directly, so swapping the Linux
/// backend for a future SHER-Kernel-native one changes nothing about how pointer
/// speed or layout selection work.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InputConfig {
    pub keyboard: KeyboardConfig,
    pub pointer: PointerConfig,
    pub touch: TouchConfig,
    pub accessibility: AccessibilityConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyboardConfig {
    pub layout: String,
    pub repeat_delay_ms: u32,
    pub repeat_rate_hz: u32,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            layout: "us".to_string(),
            repeat_delay_ms: 500,
            repeat_rate_hz: 25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointerConfig {
    pub speed: f64,
    pub acceleration: f64,
    pub natural_scrolling: bool,
    pub left_handed: bool,
}

impl Default for PointerConfig {
    fn default() -> Self {
        Self {
            speed: 1.0,
            acceleration: 1.0,
            natural_scrolling: false,
            left_handed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TouchConfig {
    pub tap_to_click: bool,
    pub palm_rejection: bool,
}

impl Default for TouchConfig {
    fn default() -> Self {
        Self {
            tap_to_click: true,
            palm_rejection: true,
        }
    }
}

/// Section 21: flags SHER-Input must carry from Phase 1 even though most of the
/// behavior they imply (bounce-key debouncing, dwell click, mouse-keys emulation) is
/// implemented in later phases — adding the flag later would mean plumbing it through
/// every backend and the config-loading path a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    pub sticky_keys: bool,
    pub slow_keys: bool,
    pub bounce_keys: bool,
    pub mouse_keys: bool,
}
