use crate::device::InputDevice;
use crate::gamepad::GamepadEvent;
use crate::ids::{InputDeviceId, SequenceNumber, Timestamp};
use crate::keyboard::{KeyboardEvent, Modifiers};
use crate::pointer::PointerEvent;
use crate::source::InputSource;
use crate::tablet::TabletEvent;
use crate::touch::TouchEvent;
use serde::{Deserialize, Serialize};

/// The payload of an [`InputEvent`]. New device classes add a variant here; existing
/// consumers that match exhaustively on unrelated variants are unaffected (section 11:
/// "adding a new input device [must not] require rewriting the entire event system").
/// `DeviceAdded`/`DeviceRemoved` live in this enum too rather than a side channel —
/// hotplug is a normal event in the same ordered, sequenced stream (section 9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEventPayload {
    Keyboard(KeyboardEvent),
    Pointer(PointerEvent),
    Touch(TouchEvent),
    Tablet(TabletEvent),
    Gamepad(GamepadEvent),
    DeviceAdded(InputDevice),
    DeviceRemoved(InputDeviceId),
}

/// The canonical SHER input event (section 11). Every event carries enough to place it
/// in the global order (`timestamp` + `sequence`, section 12), attribute it to a
/// device, and know whether it came from hardware or a trusted synthetic source
/// (section 23) — regardless of payload kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputEvent {
    pub timestamp: Timestamp,
    pub sequence: SequenceNumber,
    pub device_id: InputDeviceId,
    pub source: InputSource,
    pub modifiers: Modifiers,
    pub payload: InputEventPayload,
}

impl InputEvent {
    /// `true` for high-frequency motion/scroll events that
    /// [`InputService`](crate::service::InputService) is allowed to coalesce under
    /// backpressure (section 14). Button/key/touch transitions always return `false` —
    /// coalescing them would silently drop a press or release.
    pub fn is_coalescible(&self) -> bool {
        matches!(
            self.payload,
            InputEventPayload::Pointer(PointerEvent::MotionRelative { .. })
                | InputEventPayload::Pointer(PointerEvent::MotionAbsolute { .. })
                | InputEventPayload::Pointer(PointerEvent::Scroll { .. })
        )
    }
}
