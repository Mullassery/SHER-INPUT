use crate::linux::{device, reader};
use sher_input_core::{BackendEvent, BackendEventSink, Error, InputBackend, InputDeviceId, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// SHER-Input's first backend (section 25): translates `/dev/input/eventN` nodes into
/// the canonical [`sher_input_core`] model. Everything Linux-flavored — evdev types,
/// event-node paths, keycodes — is confined to this crate; `sher_input_core` and
/// everything above it never sees them (section 3).
#[derive(Default)]
pub struct LinuxBackend;

impl LinuxBackend {
    pub fn new() -> Self {
        Self
    }
}

impl InputBackend for LinuxBackend {
    fn name(&self) -> &'static str {
        "sher_input_linux::LinuxBackend"
    }

    fn spawn(
        self: Box<Self>,
        sink: BackendEventSink,
        running: Arc<AtomicBool>,
    ) -> Result<std::thread::JoinHandle<()>> {
        std::thread::Builder::new()
            .name("sher-input-linux-supervisor".to_string())
            .spawn(move || supervise(sink, running))
            .map_err(|e| Error::Backend(e.to_string()))
    }
}

/// Tracks which path maps to which minted [`InputDeviceId`] so a later disappearance
/// of that path can be reported as `DeviceRemoved` for the right device (section 9).
/// Per-device reader threads are intentionally not joined here — see
/// `reader::run`'s doc comment for why — so hotplug polling (bounded by
/// `POLL_INTERVAL`) is the only thing this loop needs to stay responsive to `running`.
fn supervise(sink: BackendEventSink, running: Arc<AtomicBool>) {
    let mut known: HashMap<PathBuf, InputDeviceId> = HashMap::new();

    scan_and_report_new(&sink, &running, &mut known);

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(POLL_INTERVAL);
        if !running.load(Ordering::SeqCst) {
            break;
        }
        scan_and_report_new(&sink, &running, &mut known);
        report_removed(&sink, &mut known);
    }
}

fn scan_and_report_new(
    sink: &BackendEventSink,
    running: &Arc<AtomicBool>,
    known: &mut HashMap<PathBuf, InputDeviceId>,
) {
    for (path, dev) in evdev::enumerate() {
        if known.contains_key(&path) {
            continue;
        }
        let described = device::describe(&path, &dev);
        let device_id = described.id;
        sink.send(BackendEvent::DeviceAdded(described));
        known.insert(path.clone(), device_id);

        let reader_sink = sink.clone();
        let reader_running = Arc::clone(running);
        let spawned = std::thread::Builder::new()
            .name(format!("sher-input-reader-{device_id}"))
            .spawn(move || reader::run(device_id, dev, reader_sink, reader_running));
        if let Err(err) = spawned {
            tracing::warn!(
                "sher_input_linux: failed to spawn reader thread for {}: {err}",
                path.display()
            );
        }
    }
}

fn report_removed(sink: &BackendEventSink, known: &mut HashMap<PathBuf, InputDeviceId>) {
    known.retain(|path, device_id| {
        if path.exists() {
            true
        } else {
            sink.send(BackendEvent::DeviceRemoved(*device_id));
            false
        }
    });
}
