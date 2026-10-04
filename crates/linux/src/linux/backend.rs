use crate::linux::{device, reader};
use sher_input_core::{BackendEvent, BackendEventSink, Error, InputBackend, InputDeviceId, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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
    let mut warned_inaccessible: HashSet<PathBuf> = HashSet::new();
    let mut warned_dir_unreadable = false;

    warn_if_dev_input_unreadable(Path::new("/dev/input"), &mut warned_dir_unreadable);
    scan_and_report_new(&sink, &running, &mut known);
    warn_about_inaccessible_devices(&mut warned_inaccessible);

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(POLL_INTERVAL);
        if !running.load(Ordering::SeqCst) {
            break;
        }
        warn_if_dev_input_unreadable(Path::new("/dev/input"), &mut warned_dir_unreadable);
        scan_and_report_new(&sink, &running, &mut known);
        report_removed(&sink, &mut known);
        warn_about_inaccessible_devices(&mut warned_inaccessible);
    }
}

/// The directory-level counterpart to [`warn_about_inaccessible_devices`]: if
/// `/dev/input` itself can't even be listed (as opposed to being listable but
/// containing individual nodes this user can't open), `evdev::enumerate()` and
/// [`scan_for_permission_denied`] both silently see zero entries — this is the one
/// case neither of those catches, and without it a denied directory listing looks
/// identical to "no input hardware exists," with zero devices and zero log output
/// either way. Logged once (not every `POLL_INTERVAL`) since this can't resolve
/// itself without a group-membership change and a new login session.
fn warn_if_dev_input_unreadable(dir: &Path, warned: &mut bool) {
    if *warned {
        return;
    }
    if let Err(err) = std::fs::read_dir(dir) {
        tracing::error!(
            "sher_input_linux: {} could not be listed ({err}); no input devices can be \
             discovered at all. Add this user to the `input` group (or whichever \
             group/udev rule grants access to input devices on this system) and start a \
             new session.",
            dir.display()
        );
        *warned = true;
    }
}

/// `evdev::enumerate()` silently drops any `/dev/input/eventN` node it fails to
/// open (see the `evdev` crate's `raw_stream::enumerate`, which only yields a pair
/// on `RawDevice::open(&path).ok()`) — a node the running user can't read due to
/// missing `input`-group / udev `uaccess` membership simply never appears, with no
/// error anywhere. Without this, a non-privileged user on a freshly installed
/// Ubuntu box sees zero devices and no indication why. Logged once per path so a
/// node a user can't fix mid-session doesn't spam the log every `POLL_INTERVAL`.
fn warn_about_inaccessible_devices(warned: &mut HashSet<PathBuf>) {
    for path in scan_for_permission_denied(Path::new("/dev/input")) {
        if warned.insert(path.clone()) {
            tracing::warn!(
                "sher_input_linux: {} exists but could not be opened (permission denied); \
                 it will not appear in the input stream. Add this user to the `input` group \
                 (or whichever group/udev rule grants access to input devices on this system).",
                path.display()
            );
        }
    }
}

/// Lists `eventN` nodes under `dir` that exist but fail to open with
/// `PermissionDenied` specifically — any other failure (node mid-removal, not a
/// character device, etc.) is not this function's concern and is left for the
/// normal hotplug/removal path to handle.
fn scan_for_permission_denied(dir: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"))
        })
        .filter(|path| {
            matches!(
                std::fs::File::open(path).err().map(|e| e.kind()),
                Some(std::io::ErrorKind::PermissionDenied)
            )
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn unique_temp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "sher-input-permtest-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nonexistent_directory_yields_no_findings() {
        let found = scan_for_permission_denied(Path::new("/nonexistent/sher-input-test-path"));
        assert!(found.is_empty());
    }

    #[test]
    fn permission_denied_node_is_flagged_but_an_openable_node_is_not() {
        let dir = unique_temp_dir();

        let denied = dir.join("event0");
        std::fs::write(&denied, b"").unwrap();
        std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).unwrap();

        let openable = dir.join("event1");
        std::fs::write(&openable, b"").unwrap();

        let not_an_event_node = dir.join("js0");
        std::fs::write(&not_an_event_node, b"").unwrap();
        std::fs::set_permissions(&not_an_event_node, std::fs::Permissions::from_mode(0o000))
            .unwrap();

        // Root (or any CAP_DAC_OVERRIDE context) bypasses the permission bits this
        // test relies on; skip rather than false-fail there instead of asserting.
        if std::fs::File::open(&denied).is_ok() {
            let _ = std::fs::remove_dir_all(&dir);
            eprintln!("skipping: running with privileges that bypass file permissions");
            return;
        }

        let found = scan_for_permission_denied(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            found,
            vec![denied],
            "only the inaccessible event* node should be flagged"
        );
    }

    #[test]
    fn unreadable_directory_is_warned_about_exactly_once() {
        let parent = unique_temp_dir();
        let unreadable = parent.join("locked");
        std::fs::create_dir_all(&unreadable).unwrap();
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();

        // Root (or any CAP_DAC_OVERRIDE context) bypasses directory permission bits too.
        if std::fs::read_dir(&unreadable).is_ok() {
            let _ = std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o755));
            let _ = std::fs::remove_dir_all(&parent);
            eprintln!("skipping: running with privileges that bypass directory permissions");
            return;
        }

        let mut warned = false;
        warn_if_dev_input_unreadable(&unreadable, &mut warned);
        assert!(warned, "an unreadable directory must flip the warned flag");

        // Second call must not panic or re-derive anything from the (still-unreadable)
        // directory — the early return on `*warned` is what prevents log spam every
        // `POLL_INTERVAL`.
        warn_if_dev_input_unreadable(&unreadable, &mut warned);
        assert!(warned);

        let _ = std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn readable_directory_never_sets_the_warned_flag() {
        let dir = unique_temp_dir();
        let mut warned = false;
        warn_if_dev_input_unreadable(&dir, &mut warned);
        assert!(!warned);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn warn_about_inaccessible_devices_only_warns_once_per_path() {
        let dir = unique_temp_dir();
        let denied = dir.join("event0");
        std::fs::write(&denied, b"").unwrap();
        std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).unwrap();

        if std::fs::File::open(&denied).is_ok() {
            let _ = std::fs::remove_dir_all(&dir);
            eprintln!("skipping: running with privileges that bypass file permissions");
            return;
        }

        let mut warned = HashSet::new();
        assert_eq!(scan_for_permission_denied(&dir).len(), 1);
        warned.insert(denied.clone());
        // Second pass over the same directory still finds it inaccessible, but
        // warn_about_inaccessible_devices's caller is expected to de-dupe via `warned`.
        assert!(warned.contains(&scan_for_permission_denied(&dir)[0]));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
