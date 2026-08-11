use sher_input_core::{BackendEventSink, Error, InputBackend, Result};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Stand-in used on any non-Linux host so the workspace still builds and tests pass
/// everywhere (this repo is developed on macOS); on Linux this module is compiled out
/// entirely in favor of `linux::LinuxBackend` (section 25).
pub struct LinuxBackend;

impl LinuxBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBackend for LinuxBackend {
    fn name(&self) -> &'static str {
        "sher_input_linux::LinuxBackend (unsupported on this platform)"
    }

    fn spawn(self: Box<Self>, _sink: BackendEventSink, _running: Arc<AtomicBool>) -> Result<std::thread::JoinHandle<()>> {
        Err(Error::Unsupported(
            "sher_input_linux requires Linux/evdev; use sher_input_test::ScriptedBackend for development on this platform"
                .to_string(),
        ))
    }
}
