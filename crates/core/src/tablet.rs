use crate::pointer::NormalizedPosition;
use serde::{Deserialize, Serialize};

/// Section 7: a dedicated stylus abstraction, present from Phase 1 so the event model
/// never needs a breaking change to add it, even though no backend implements it until
/// Phase 4 (section 34).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabletTool {
    Pen,
    Eraser,
    Brush,
    Airbrush,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TabletEvent {
    Proximity { tool: TabletTool, entering: bool },
    Motion {
        position: NormalizedPosition,
        pressure: f32,
        tilt_x: f32,
        tilt_y: f32,
    },
    Button { button: u8, pressed: bool },
}
