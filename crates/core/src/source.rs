use serde::{Deserialize, Serialize};

/// Where an event originated. Present on every [`InputEvent`](crate::event::InputEvent)
/// so consumers can *always* tell physical activity from injected activity — section 23
/// requires synthetic input to be explicitly distinguishable, never silently
/// indistinguishable from a real key press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputSource {
    /// Came from a real device through a backend (Linux evdev today, SHER-Kernel's
    /// native input backend eventually).
    Physical,
    /// Injected by a trusted caller holding a [`SyntheticInputGrant`].
    Synthetic(SyntheticOrigin),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntheticOrigin {
    AccessibilityTool,
    AutomatedTesting,
    RemoteDesktop,
    AiAgent,
    SystemTool,
}

/// Capability token required to submit synthetic input (section 22, 24). SHER-Input
/// itself does not decide *who* gets a grant — that is SHER-Kernel's capability model
/// — it only refuses to accept synthetic events without one. There is deliberately no
/// `SyntheticInputGrant::unrestricted()`: every grant is scoped to one origin and one
/// event kind, matching section 24's "an AI agent should not automatically obtain
/// unrestricted access."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticInputGrant {
    pub origin: SyntheticOrigin,
    pub allows_keyboard: bool,
    pub allows_pointer: bool,
    pub allows_touch: bool,
}

impl SyntheticInputGrant {
    pub fn new(origin: SyntheticOrigin) -> Self {
        Self {
            origin,
            allows_keyboard: false,
            allows_pointer: false,
            allows_touch: false,
        }
    }

    pub fn with_keyboard(mut self) -> Self {
        self.allows_keyboard = true;
        self
    }

    pub fn with_pointer(mut self) -> Self {
        self.allows_pointer = true;
        self
    }

    pub fn with_touch(mut self) -> Self {
        self.allows_touch = true;
        self
    }
}
