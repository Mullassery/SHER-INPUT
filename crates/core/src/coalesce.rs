use crate::event::{InputEvent, InputEventPayload};
use crate::ids::InputDeviceId;
use crate::pointer::{PointerEvent, ScrollAxis};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CoalesceKey {
    Motion,
    Scroll(ScrollAxis),
}

fn coalesce_key(event: &InputEvent) -> Option<CoalesceKey> {
    match &event.payload {
        InputEventPayload::Pointer(PointerEvent::MotionRelative { .. })
        | InputEventPayload::Pointer(PointerEvent::MotionAbsolute { .. }) => {
            Some(CoalesceKey::Motion)
        }
        InputEventPayload::Pointer(PointerEvent::Scroll { axis, .. }) => {
            Some(CoalesceKey::Scroll(*axis))
        }
        _ => None,
    }
}

fn merge(existing: &mut InputEvent, incoming: InputEvent) {
    match (&mut existing.payload, incoming.payload) {
        (
            InputEventPayload::Pointer(PointerEvent::MotionRelative { dx, dy }),
            InputEventPayload::Pointer(PointerEvent::MotionRelative { dx: idx, dy: idy }),
        ) => {
            *dx += idx;
            *dy += idy;
        }
        (
            InputEventPayload::Pointer(PointerEvent::Scroll { delta, .. }),
            InputEventPayload::Pointer(PointerEvent::Scroll { delta: idelta, .. }),
        ) => {
            *delta += idelta;
        }
        // Absolute motion and any other coalescible kind: the newest sample always
        // supersedes the last one entirely rather than accumulating.
        (existing_payload, incoming_payload) => *existing_payload = incoming_payload,
    }
    existing.timestamp = incoming.timestamp;
    existing.sequence = incoming.sequence;
    existing.modifiers = incoming.modifiers;
}

/// Implements section 14's controlled coalescing: high-frequency `PointerMove`/
/// `PointerScroll` samples for the same device may be merged while a consumer is busy,
/// but `ButtonDown`/`ButtonUp`/`KeyDown`/`KeyUp`/touch transitions never are — and
/// merging never reorders a coalesced motion sample past a button event that logically
/// followed it, because [`Coalescer::offer`] flushes pending motion before returning a
/// non-coalescible event.
#[derive(Debug, Default)]
pub struct Coalescer {
    pending: HashMap<(InputDeviceId, CoalesceKey), InputEvent>,
}

impl Coalescer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the events that are now safe to deliver, in order. A coalescible event
    /// returns an empty vec (it becomes, or merges into, the pending sample for its
    /// device+kind). A non-coalescible event flushes any pending sample for that
    /// device first, preserving ordering.
    pub fn offer(&mut self, event: InputEvent) -> Vec<InputEvent> {
        match coalesce_key(&event) {
            Some(key) => {
                self.pending
                    .entry((event.device_id, key))
                    .and_modify(|existing| merge(existing, event.clone()))
                    .or_insert(event);
                Vec::new()
            }
            None => {
                let mut out: Vec<InputEvent> = self.drain_device(event.device_id);
                out.push(event);
                out
            }
        }
    }

    /// Flushes all pending samples for one device, e.g. before delivering a
    /// non-coalescible event or when the device is removed.
    pub fn drain_device(&mut self, device_id: InputDeviceId) -> Vec<InputEvent> {
        let keys: Vec<_> = self
            .pending
            .keys()
            .filter(|(id, _)| *id == device_id)
            .cloned()
            .collect();
        let mut out: Vec<_> = keys
            .into_iter()
            .filter_map(|k| self.pending.remove(&k))
            .collect();
        out.sort_by_key(|e| e.sequence);
        out
    }

    /// Flushes every pending sample across all devices, e.g. on a compositor frame
    /// tick so coalesced motion never waits longer than one frame to arrive.
    pub fn drain_all(&mut self) -> Vec<InputEvent> {
        let mut out: Vec<_> = self.pending.drain().map(|(_, v)| v).collect();
        out.sort_by_key(|e| e.sequence);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{SequenceNumber, Timestamp};
    use crate::keyboard::{KeyAction, KeyboardEvent, LogicalKey, Modifiers, PhysicalKey};
    use crate::pointer::PointerButton;
    use crate::source::InputSource;

    fn base(device_id: InputDeviceId, seq: u64, payload: InputEventPayload) -> InputEvent {
        InputEvent {
            timestamp: Timestamp(seq),
            sequence: SequenceNumber(seq),
            device_id,
            source: InputSource::Physical,
            modifiers: Modifiers::default(),
            payload,
        }
    }

    fn motion(device_id: InputDeviceId, seq: u64, dx: f64, dy: f64) -> InputEvent {
        base(
            device_id,
            seq,
            InputEventPayload::Pointer(PointerEvent::MotionRelative { dx, dy }),
        )
    }

    fn button(device_id: InputDeviceId, seq: u64, pressed: bool) -> InputEvent {
        base(
            device_id,
            seq,
            InputEventPayload::Pointer(PointerEvent::Button {
                button: PointerButton::Left,
                pressed,
            }),
        )
    }

    fn key(device_id: InputDeviceId, seq: u64) -> InputEvent {
        base(
            device_id,
            seq,
            InputEventPayload::Keyboard(KeyboardEvent {
                physical_key: PhysicalKey::KeyA,
                logical_key: LogicalKey::Character('a'),
                text: Some("a".to_string()),
                action: KeyAction::Down,
            }),
        )
    }

    #[test]
    fn consecutive_motion_is_merged_not_emitted() {
        let mut c = Coalescer::new();
        let device_id = InputDeviceId::new();

        assert!(c.offer(motion(device_id, 0, 1.0, 1.0)).is_empty());
        assert!(c.offer(motion(device_id, 1, 2.0, 2.0)).is_empty());

        let flushed = c.drain_device(device_id);
        assert_eq!(flushed.len(), 1);
        match &flushed[0].payload {
            InputEventPayload::Pointer(PointerEvent::MotionRelative { dx, dy }) => {
                assert_eq!(*dx, 3.0);
                assert_eq!(*dy, 3.0);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn button_events_are_never_coalesced() {
        let mut c = Coalescer::new();
        let device_id = InputDeviceId::new();

        let first = c.offer(button(device_id, 0, true));
        let second = c.offer(button(device_id, 1, false));

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn key_events_are_never_coalesced() {
        let mut c = Coalescer::new();
        let device_id = InputDeviceId::new();

        assert_eq!(c.offer(key(device_id, 0)).len(), 1);
        assert_eq!(c.offer(key(device_id, 1)).len(), 1);
    }

    #[test]
    fn pending_motion_flushes_before_a_later_button_event() {
        let mut c = Coalescer::new();
        let device_id = InputDeviceId::new();

        assert!(c.offer(motion(device_id, 0, 5.0, 0.0)).is_empty());
        let delivered = c.offer(button(device_id, 1, true));

        assert_eq!(delivered.len(), 2);
        assert!(matches!(
            delivered[0].payload,
            InputEventPayload::Pointer(PointerEvent::MotionRelative { .. })
        ));
        assert!(matches!(
            delivered[1].payload,
            InputEventPayload::Pointer(PointerEvent::Button { .. })
        ));
    }

    #[test]
    fn different_devices_do_not_coalesce_together() {
        let mut c = Coalescer::new();
        let a = InputDeviceId::new();
        let b = InputDeviceId::new();

        c.offer(motion(a, 0, 1.0, 0.0));
        c.offer(motion(b, 1, 1.0, 0.0));

        assert_eq!(c.drain_device(a).len(), 1);
        assert_eq!(c.drain_device(b).len(), 1);
    }
}
