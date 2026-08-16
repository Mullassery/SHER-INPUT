use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    Select,
    Start,
    LeftStick,
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Guide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

/// Raw controller input, as reported by the device. Kept separate from any future
/// logical action-mapping layer (section 8: "raw controller input" vs "logical
/// game-controller actions") — that mapping belongs to the application/game, not to
/// SHER-Input, which only guarantees the raw signal is normalized and timestamped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GamepadEvent {
    Button {
        button: GamepadButton,
        pressed: bool,
    },
    /// Normalized to `[-1.0, 1.0]` for sticks and `[0.0, 1.0]` for triggers.
    Axis { axis: GamepadAxis, value: f32 },
}
