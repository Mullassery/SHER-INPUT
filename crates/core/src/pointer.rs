use crate::ids::InputDeviceId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

/// A position in SHER-Input's own normalized coordinate space: device units resolved
/// to a consistent origin, but *not yet* mapped onto a specific output or surface —
/// that mapping is SHER-Display's job (section 18). SHER-Input never emits "surface
/// coordinates."
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPosition {
    pub x: f64,
    pub y: f64,
}

/// Relative motion (mice, most touchpads) is kept structurally distinct from absolute
/// position (touchscreens, tablets, section 5) — a pointer event is one or the other,
/// never a lossy merge of both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerEvent {
    MotionRelative { dx: f64, dy: f64 },
    MotionAbsolute { position: NormalizedPosition },
    Button { button: PointerButton, pressed: bool },
    Scroll { axis: ScrollAxis, delta: f64, high_resolution: bool },
    Enter,
    Leave,
}

/// Live per-pointer-device state: button set and last known position, so
/// [`InputEvent`](crate::event::InputEvent) modifiers/position can be reconstructed
/// without re-deriving them from event history.
#[derive(Debug, Default)]
pub struct PointerState {
    position: NormalizedPosition,
    buttons: HashSet<PointerButton>,
}

impl Default for NormalizedPosition {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl PointerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn position(&self) -> NormalizedPosition {
        self.position
    }

    pub fn is_pressed(&self, button: PointerButton) -> bool {
        self.buttons.contains(&button)
    }

    pub fn apply_relative(&mut self, dx: f64, dy: f64) {
        self.position.x += dx;
        self.position.y += dy;
    }

    pub fn apply_absolute(&mut self, position: NormalizedPosition) {
        self.position = position;
    }

    pub fn note_button(&mut self, button: PointerButton, pressed: bool) {
        if pressed {
            self.buttons.insert(button);
        } else {
            self.buttons.remove(&button);
        }
    }
}

#[derive(Debug, Default)]
pub struct PointerStateTable(HashMap<InputDeviceId, PointerState>);

impl PointerStateTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state_mut(&mut self, device: InputDeviceId) -> &mut PointerState {
        self.0.entry(device).or_default()
    }

    pub fn remove(&mut self, device: InputDeviceId) {
        self.0.remove(&device);
    }
}
