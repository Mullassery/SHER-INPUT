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
pub fn run(device_id: InputDeviceId, mut device: Device, sink: BackendEventSink, running: Arc<AtomicBool>) {
    while running.load(Ordering::SeqCst) {
        match device.fetch_events() {
            Ok(events) => translate_batch(device_id, events, &sink),
            Err(err) => {
                tracing::warn!("sher_input_linux: device {device_id} read error, treating as unplugged: {err}");
                sink.send(BackendEvent::DeviceRemoved(device_id));
                return;
            }
        }
    }
}

/// Relative-motion axes arrive as separate `REL_X`/`REL_Y` events within one kernel
/// report; they are accumulated and emitted as a single [`BackendEvent::MotionRelative`]
/// so `sher_input_core`'s coalescer sees one sample per report rather than two
/// artificially-split ones.
fn translate_batch(device_id: InputDeviceId, events: impl Iterator<Item = evdev::InputEvent>, sink: &BackendEventSink) {
    let mut dx = 0.0_f64;
    let mut dy = 0.0_f64;
    let mut has_motion = false;

    for ev in events {
        match ev.kind() {
            InputEventKind::Key(key) => {
                let pressed = ev.value() != 0;
                if let Some(button) = keymap::map_button(key) {
                    sink.send(BackendEvent::PointerButton { device_id, button, pressed });
                } else {
                    sink.send(BackendEvent::Key { device_id, physical_key: keymap::map_key(key), pressed });
                }
            }
            InputEventKind::RelAxis(axis) => match axis {
                RelativeAxisType::REL_X => {
                    dx += ev.value() as f64;
                    has_motion = true;
                }
                RelativeAxisType::REL_Y => {
                    dy += ev.value() as f64;
                    has_motion = true;
                }
                RelativeAxisType::REL_WHEEL => sink.send(BackendEvent::Scroll {
                    device_id,
                    axis: ScrollAxis::Vertical,
                    delta: ev.value() as f64,
                    high_resolution: false,
                }),
                RelativeAxisType::REL_HWHEEL => sink.send(BackendEvent::Scroll {
                    device_id,
                    axis: ScrollAxis::Horizontal,
                    delta: ev.value() as f64,
                    high_resolution: false,
                }),
                RelativeAxisType::REL_WHEEL_HI_RES => sink.send(BackendEvent::Scroll {
                    device_id,
                    axis: ScrollAxis::Vertical,
                    delta: ev.value() as f64 / 120.0,
                    high_resolution: true,
                }),
                RelativeAxisType::REL_HWHEEL_HI_RES => sink.send(BackendEvent::Scroll {
                    device_id,
                    axis: ScrollAxis::Horizontal,
                    delta: ev.value() as f64 / 120.0,
                    high_resolution: true,
                }),
                _ => {}
            },
            InputEventKind::Synchronization(_) => {
                if has_motion {
                    sink.send(BackendEvent::MotionRelative { device_id, dx, dy });
                    dx = 0.0;
                    dy = 0.0;
                    has_motion = false;
                }
            }
            // EV_ABS (touch/tablet), EV_MSC, EV_LED, ... : normalized in a later
            // phase (section 34); intentionally not translated yet.
            _ => {}
        }
    }

    if has_motion {
        sink.send(BackendEvent::MotionRelative { device_id, dx, dy });
    }
}
