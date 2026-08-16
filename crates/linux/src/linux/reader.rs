use crate::linux::keymap;
use evdev::{Device, InputEventKind, RelativeAxisType};
use sher_input_core::{BackendEvent, BackendEventSink, InputDeviceId, ScrollAxis};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Blocking per-device read loop (one OS thread per evdev node). Blocking on
/// `fetch_events()` is deliberate: it means this thread costs nothing while idle and
/// wakes exactly when the kernel has something to report — no polling on the hot path
/// (section 13). The `running` flag is only checked between batches; a device with no
/// further activity keeps this thread parked past shutdown until it errors out or the
/// process exits, which is an accepted limitation until a `mio`/`epoll`-based version
/// gives every reader a wakeup edge (tracked as a Phase 2 follow-up, not required for
/// Phase 1's scope).
pub fn run(
    device_id: InputDeviceId,
    mut device: Device,
    sink: BackendEventSink,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        match device.fetch_events() {
            Ok(events) => translate_batch(device_id, events, &sink),
            Err(err) => {
                tracing::warn!(
                    "sher_input_linux: device {device_id} read error, treating as unplugged: {err}"
                );
                sink.send(BackendEvent::DeviceRemoved(device_id));
                return;
            }
        }
    }
}

/// Accumulates `REL_X`/`REL_Y` until told to stop, so a batch that contains several
/// motion samples can still be emitted as one [`BackendEvent::MotionRelative`].
#[derive(Default)]
struct PendingMotion {
    dx: f64,
    dy: f64,
    has_motion: bool,
}

impl PendingMotion {
    /// Sends and clears the accumulated sample, if any. Called before *every*
    /// non-motion event this batch produces (button, key, scroll, or an explicit
    /// `SYN_REPORT`) — not only at `SYN_REPORT` — because a single `fetch_events()`
    /// batch can contain `REL_X`/`REL_Y` followed by a `Key` event *before* the kernel
    /// sends `SYN_REPORT`. Flushing only on `Synchronization` would send that button
    /// event to the sink first and the motion that physically preceded it second,
    /// inverting the order `InputService` sequences them in (section 12/14: ordering
    /// must reflect what actually happened, coalescing must never reorder a
    /// non-coalescible event ahead of the motion it followed).
    fn flush(&mut self, device_id: InputDeviceId, sink: &BackendEventSink) {
        if self.has_motion {
            sink.send(BackendEvent::MotionRelative {
                device_id,
                dx: self.dx,
                dy: self.dy,
            });
            self.dx = 0.0;
            self.dy = 0.0;
            self.has_motion = false;
        }
    }
}

/// Relative-motion axes arrive as separate `REL_X`/`REL_Y` events within one kernel
/// report; they are accumulated and emitted as a single [`BackendEvent::MotionRelative`]
/// so `sher_input_core`'s coalescer sees one sample per report rather than two
/// artificially-split ones.
fn translate_batch(
    device_id: InputDeviceId,
    events: impl Iterator<Item = evdev::InputEvent>,
    sink: &BackendEventSink,
) {
    let mut motion = PendingMotion::default();

    for ev in events {
        match ev.kind() {
            InputEventKind::Key(key) => {
                motion.flush(device_id, sink);
                let pressed = ev.value() != 0;
                if let Some(button) = keymap::map_button(key) {
                    sink.send(BackendEvent::PointerButton {
                        device_id,
                        button,
                        pressed,
                    });
                } else {
                    sink.send(BackendEvent::Key {
                        device_id,
                        physical_key: keymap::map_key(key),
                        pressed,
                    });
                }
            }
            InputEventKind::RelAxis(axis) => match axis {
                RelativeAxisType::REL_X => {
                    motion.dx += ev.value() as f64;
                    motion.has_motion = true;
                }
                RelativeAxisType::REL_Y => {
                    motion.dy += ev.value() as f64;
                    motion.has_motion = true;
                }
                RelativeAxisType::REL_WHEEL => {
                    motion.flush(device_id, sink);
                    sink.send(BackendEvent::Scroll {
                        device_id,
                        axis: ScrollAxis::Vertical,
                        delta: ev.value() as f64,
                        high_resolution: false,
                    });
                }
                RelativeAxisType::REL_HWHEEL => {
                    motion.flush(device_id, sink);
                    sink.send(BackendEvent::Scroll {
                        device_id,
                        axis: ScrollAxis::Horizontal,
                        delta: ev.value() as f64,
                        high_resolution: false,
                    });
                }
                RelativeAxisType::REL_WHEEL_HI_RES => {
                    motion.flush(device_id, sink);
                    sink.send(BackendEvent::Scroll {
                        device_id,
                        axis: ScrollAxis::Vertical,
                        delta: ev.value() as f64 / 120.0,
                        high_resolution: true,
                    });
                }
                RelativeAxisType::REL_HWHEEL_HI_RES => {
                    motion.flush(device_id, sink);
                    sink.send(BackendEvent::Scroll {
                        device_id,
                        axis: ScrollAxis::Horizontal,
                        delta: ev.value() as f64 / 120.0,
                        high_resolution: true,
                    });
                }
                _ => {}
            },
            InputEventKind::Synchronization(_) => motion.flush(device_id, sink),
            // EV_ABS (touch/tablet), EV_MSC, EV_LED, ... : normalized in a later
            // phase (section 34); intentionally not translated yet.
            _ => {}
        }
    }

    motion.flush(device_id, sink);
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::{EventType, InputEvent as RawInputEvent, Key as EvKey, Synchronization};
    use sher_input_core::{PhysicalKey, PointerButton};
    use tokio::sync::mpsc;

    fn harness() -> (
        InputDeviceId,
        BackendEventSink,
        mpsc::UnboundedReceiver<BackendEvent>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        (InputDeviceId::new(), BackendEventSink::new(tx), rx)
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<BackendEvent>) -> Vec<BackendEvent> {
        let mut out = Vec::new();
        while let Ok(event) = rx.try_recv() {
            out.push(event);
        }
        out
    }

    /// Real evdev wire bytes (well, the parsed `input_event` struct they decode to) for
    /// two relative-motion samples inside one kernel report, exactly as a mouse reports
    /// diagonal motion: `REL_X`, `REL_Y`, then `SYN_REPORT` terminates the report.
    #[test]
    fn rel_x_and_rel_y_within_one_report_coalesce_into_one_motion_event() {
        let (device_id, sink, mut rx) = harness();
        let batch = [
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, 12),
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, -4),
            RawInputEvent::new(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0),
        ];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BackendEvent::MotionRelative { dx, dy, .. } => {
                assert_eq!(*dx, 12.0);
                assert_eq!(*dy, -4.0);
            }
            other => panic!("expected MotionRelative, got {other:?}"),
        }
    }

    /// A slow reader can end up with `fetch_events()` returning several kernel reports
    /// in one call; motion across all of them for the same device must still merge into
    /// a single sample rather than emitting one per `SYN_REPORT`.
    #[test]
    fn motion_across_multiple_syn_terminated_reports_in_one_batch_accumulates() {
        let (device_id, sink, mut rx) = harness();
        let syn = RawInputEvent::new(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0);
        let batch = [
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, 1),
            syn,
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, 2),
            syn,
        ];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 2, "each SYN_REPORT flushes its own sample");
        for event in &events {
            assert!(matches!(event, BackendEvent::MotionRelative { .. }));
        }
    }

    /// The real bug this guards: a mouse click-and-drag can land `REL_X`, `REL_Y`, and
    /// the button's `Key` event inside the *same* kernel report before `SYN_REPORT` —
    /// motion physically happened first and must be flushed to the sink first, or
    /// `InputService` sequences the click ahead of the drag that produced it.
    #[test]
    fn motion_is_flushed_before_a_button_event_in_the_same_unsynced_batch() {
        let (device_id, sink, mut rx) = harness();
        let batch = [
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, 5),
            RawInputEvent::new(EventType::KEY, EvKey::BTN_LEFT.code(), 1),
        ];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 2);
        assert!(
            matches!(events[0], BackendEvent::MotionRelative { .. }),
            "motion must be observed before the button that followed it, got {:?}",
            events[0]
        );
        assert!(matches!(
            events[1],
            BackendEvent::PointerButton {
                button: PointerButton::Left,
                pressed: true,
                ..
            }
        ));
    }

    /// Same ordering hazard, but with a scroll sample instead of a button.
    #[test]
    fn motion_is_flushed_before_a_scroll_event_in_the_same_unsynced_batch() {
        let (device_id, sink, mut rx) = harness();
        let batch = [
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, 3),
            RawInputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_WHEEL.0, 1),
        ];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], BackendEvent::MotionRelative { .. }));
        assert!(matches!(events[1], BackendEvent::Scroll { .. }));
    }

    /// A device unplugged (or a reader shut down) mid-report never sends a trailing
    /// `SYN_REPORT` — motion accumulated so far must still reach the sink rather than
    /// being silently dropped.
    #[test]
    fn trailing_motion_without_a_final_syn_report_still_flushes() {
        let (device_id, sink, mut rx) = harness();
        let batch = [RawInputEvent::new(
            EventType::RELATIVE,
            RelativeAxisType::REL_X.0,
            7,
        )];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], BackendEvent::MotionRelative { dx, .. } if dx == 7.0));
    }

    #[test]
    fn a_letter_key_is_translated_as_a_key_event_not_a_pointer_button() {
        let (device_id, sink, mut rx) = harness();
        let batch = [RawInputEvent::new(EventType::KEY, EvKey::KEY_A.code(), 1)];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            BackendEvent::Key {
                physical_key: PhysicalKey::KeyA,
                pressed: true,
                ..
            }
        ));
    }

    #[test]
    fn high_resolution_scroll_is_scaled_down_by_120() {
        let (device_id, sink, mut rx) = harness();
        let batch = [RawInputEvent::new(
            EventType::RELATIVE,
            RelativeAxisType::REL_WHEEL_HI_RES.0,
            120,
        )];

        translate_batch(device_id, batch.into_iter(), &sink);

        let events = drain(&mut rx);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BackendEvent::Scroll {
                delta,
                high_resolution,
                ..
            } => {
                assert_eq!(*delta, 1.0);
                assert!(*high_resolution);
            }
            other => panic!("expected Scroll, got {other:?}"),
        }
    }
}
