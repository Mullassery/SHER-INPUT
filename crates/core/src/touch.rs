use crate::ids::InputDeviceId;
use crate::pointer::NormalizedPosition;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifies one finger for the duration of its contact with the surface. Assigned
/// by the backend (Linux evdev's `ABS_MT_TRACKING_ID` today) and stable from
/// `TouchPhase::Down` to its matching `Up`/`Cancel` — section 6's running example of
/// three simultaneous, independently-tracked contacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContactId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
    /// The contact was invalidated by the system (e.g. palm rejection) rather than
    /// lifted by the user — consumers must treat this like `Up` for state cleanup but
    /// may skip "tap" gesture recognition (section 6).
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchEvent {
    pub contact_id: ContactId,
    pub phase: TouchPhase,
    pub position: NormalizedPosition,
    pub pressure: Option<f32>,
    pub size: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchContact {
    pub position: NormalizedPosition,
    pub pressure: Option<f32>,
    pub size: Option<(f32, f32)>,
}

/// Tracks every live contact on one touch-capable device. A single physical touch
/// event from the backend never represents "the" touch — it is always keyed by
/// [`ContactId`] against this table (section 6: "do not assume a touch event
/// represents only one finger").
#[derive(Debug, Default)]
pub struct TouchState {
    contacts: HashMap<ContactId, TouchContact>,
}

impl TouchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_contacts(&self) -> impl Iterator<Item = (&ContactId, &TouchContact)> {
        self.contacts.iter()
    }

    pub fn contact_count(&self) -> usize {
        self.contacts.len()
    }

    pub fn apply(&mut self, event: &TouchEvent) {
        match event.phase {
            TouchPhase::Down | TouchPhase::Move => {
                self.contacts.insert(
                    event.contact_id,
                    TouchContact {
                        position: event.position,
                        pressure: event.pressure,
                        size: event.size,
                    },
                );
            }
            TouchPhase::Up | TouchPhase::Cancel => {
                self.contacts.remove(&event.contact_id);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct TouchStateTable(HashMap<InputDeviceId, TouchState>);

impl TouchStateTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state_mut(&mut self, device: InputDeviceId) -> &mut TouchState {
        self.0.entry(device).or_default()
    }

    pub fn remove(&mut self, device: InputDeviceId) {
        self.0.remove(&device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(id: u32, phase: TouchPhase) -> TouchEvent {
        TouchEvent {
            contact_id: ContactId(id),
            phase,
            position: NormalizedPosition { x: 0.0, y: 0.0 },
            pressure: None,
            size: None,
        }
    }

    #[test]
    fn tracks_multiple_simultaneous_contacts() {
        let mut state = TouchState::new();
        state.apply(&touch(14, TouchPhase::Down));
        state.apply(&touch(18, TouchPhase::Down));
        state.apply(&touch(23, TouchPhase::Down));

        assert_eq!(state.contact_count(), 3);
        assert!(state.active_contacts().any(|(id, _)| *id == ContactId(14)));
        assert!(state.active_contacts().any(|(id, _)| *id == ContactId(18)));
        assert!(state.active_contacts().any(|(id, _)| *id == ContactId(23)));
    }

    #[test]
    fn lifting_one_finger_does_not_affect_others() {
        let mut state = TouchState::new();
        state.apply(&touch(14, TouchPhase::Down));
        state.apply(&touch(18, TouchPhase::Down));

        state.apply(&touch(14, TouchPhase::Up));

        assert_eq!(state.contact_count(), 1);
        assert!(state.active_contacts().any(|(id, _)| *id == ContactId(18)));
    }

    #[test]
    fn cancel_removes_contact_like_up() {
        let mut state = TouchState::new();
        state.apply(&touch(1, TouchPhase::Down));
        state.apply(&touch(1, TouchPhase::Cancel));
        assert_eq!(state.contact_count(), 0);
    }

    #[test]
    fn move_updates_position_without_changing_contact_count() {
        let mut state = TouchState::new();
        state.apply(&touch(1, TouchPhase::Down));
        let mut moved = touch(1, TouchPhase::Move);
        moved.position = NormalizedPosition { x: 42.0, y: 7.0 };
        state.apply(&moved);

        assert_eq!(state.contact_count(), 1);
        let (_, contact) = state.active_contacts().next().unwrap();
        assert_eq!(contact.position.x, 42.0);
    }
}
