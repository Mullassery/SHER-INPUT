use crate::device::InputDevice;
use crate::error::Result;
use crate::gamepad::GamepadEvent;
use crate::ids::InputDeviceId;
use crate::pointer::{NormalizedPosition, PointerButton, ScrollAxis};
use crate::tablet::TabletEvent;
use crate::touch::TouchEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// What a backend reports, before [`InputService`](crate::service::InputService)
/// applies layout mapping, state tracking, sequencing, and coalescing. This is
/// intentionally more primitive than [`InputEvent`](crate::event::InputEvent) — a
/// backend knows "physical key 30 went down on device X," it does not know the active
/// keyboard layout or the device's current modifier state, both of which are
/// SHER-Input-core concerns (section 3, section 25: backend code stays Linux/HID-
/// flavored, everything layout- and state-aware lives above it).
#[derive(Debug, Clone)]
pub enum BackendEvent {
    DeviceAdded(InputDevice),
    DeviceRemoved(InputDeviceId),
    Key {
        device_id: InputDeviceId,
        physical_key: crate::keyboard::PhysicalKey,
        pressed: bool,
    },
    MotionRelative {
        device_id: InputDeviceId,
        dx: f64,
        dy: f64,
    },
    MotionAbsolute {
        device_id: InputDeviceId,
        position: NormalizedPosition,
    },
    PointerButton {
        device_id: InputDeviceId,
        button: PointerButton,
        pressed: bool,
    },
    Scroll {
        device_id: InputDeviceId,
        axis: ScrollAxis,
        delta: f64,
        high_resolution: bool,
    },
    Touch {
        device_id: InputDeviceId,
        event: TouchEvent,
    },
    Tablet {
        device_id: InputDeviceId,
        event: TabletEvent,
    },
    Gamepad {
        device_id: InputDeviceId,
        event: GamepadEvent,
    },
}

/// Handed to a backend so it can report events without holding a reference to
/// [`InputService`](crate::service::InputService) — backends depend only on
/// `sher_input_core`'s types, never on the service that consumes them (keeps the
/// dependency direction one-way, section 3).
#[derive(Clone)]
pub struct BackendEventSink(mpsc::UnboundedSender<BackendEvent>);

impl BackendEventSink {
    pub fn new(sender: mpsc::UnboundedSender<BackendEvent>) -> Self {
        Self(sender)
    }

    /// Safe to call from any thread, including a plain `std::thread` doing blocking
    /// device IO — sending never blocks and does not require a tokio runtime.
    pub fn send(&self, event: BackendEvent) {
        if self.0.send(event).is_err() {
            tracing::debug!("backend event dropped: input service no longer listening");
        }
    }
}

/// A running backend. Dropping it (or calling [`BackendHandle::stop`] explicitly)
/// signals the backend's IO loop to exit and joins it — device failure or backend
/// shutdown must never take the rest of the desktop down with it (section 27).
pub struct BackendHandle {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BackendHandle {
    pub fn new(running: Arc<AtomicBool>, thread: std::thread::JoinHandle<()>) -> Self {
        Self {
            running,
            thread: Some(thread),
        }
    }

    pub fn stop(mut self) {
        self.stop_inner();
    }

    /// Non-blocking liveness check: `true` once the backend's IO thread has exited,
    /// whether from a clean [`stop`](Self::stop)/drop or a panic. Unlike joining, this
    /// never blocks and doesn't consume the handle, so a caller can poll it on a timer
    /// to notice a crashed backend and decide to hot-restart it (a fresh
    /// [`InputBackend`] instance through [`InputService::start_backend`]
    /// (crate::service::InputService::start_backend)) without taking `InputService`
    /// itself down — section 27's crash-isolation claim, made pollable rather than
    /// just structurally true.
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }

    /// Stops the backend (a no-op if its thread had already exited) and reports
    /// whether that exit was a panic rather than a clean return — the distinction a
    /// hot-restart policy needs (restart on crash, don't treat an intentional `stop()`
    /// as one). Consumes the handle because `JoinHandle::join`, the only way `std`
    /// exposes this, does too.
    pub fn stop_and_check_panicked(mut self) -> bool {
        self.running.store(false, Ordering::SeqCst);
        self.thread
            .take()
            .map(|t| t.join().is_err())
            .unwrap_or(false)
    }

    fn stop_inner(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for BackendHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// The contract every backend implements (section 25: "keep Linux-specific code
/// isolated" — this trait is the isolation boundary). `sher_input_linux` implements it
/// today; a future SHER-Kernel-native backend implements the same trait and Aurora
/// notices nothing.
pub trait InputBackend: Send + 'static {
    fn name(&self) -> &'static str;

    /// Spawns whatever IO the backend needs (a thread reading evdev fds, a
    /// simulated event generator, ...) and starts reporting [`BackendEvent`]s to
    /// `sink`. The backend must observe `running` and exit promptly when it flips to
    /// `false`.
    fn spawn(
        self: Box<Self>,
        sink: BackendEventSink,
        running: Arc<AtomicBool>,
    ) -> Result<std::thread::JoinHandle<()>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait_until_finished(handle: &BackendHandle, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while !handle.is_finished() {
            assert!(Instant::now() < deadline, "did not finish in time");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn is_finished_is_false_while_the_thread_is_alive() {
        let running = Arc::new(AtomicBool::new(true));
        let running_for_thread = Arc::clone(&running);
        let thread = std::thread::spawn(move || {
            while running_for_thread.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let handle = BackendHandle::new(running, thread);

        assert!(!handle.is_finished());
        assert!(!handle.stop_and_check_panicked());
    }

    #[test]
    fn stop_and_check_panicked_is_false_for_a_clean_exit() {
        let running = Arc::new(AtomicBool::new(true));
        let thread = std::thread::spawn(|| {});
        let handle = BackendHandle::new(running, thread);

        wait_until_finished(&handle, Duration::from_secs(2));
        assert!(!handle.stop_and_check_panicked());
    }

    #[test]
    fn stop_and_check_panicked_is_true_after_a_panic() {
        let running = Arc::new(AtomicBool::new(true));
        let thread = std::thread::spawn(|| panic!("boom"));
        let handle = BackendHandle::new(running, thread);

        wait_until_finished(&handle, Duration::from_secs(2));
        assert!(handle.stop_and_check_panicked());
    }
}
