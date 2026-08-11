use sher_input_core::{BackendEvent, BackendEventSink, Error, InputBackend, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A real (if fake) [`InputBackend`] implementation: replays a fixed script of
/// timed events on its own thread, exactly like a hardware backend would report real
/// activity. Exists to prove backend pluggability end-to-end (section 3, 25) and to
/// give `sher-input-monitor` something to show without hardware attached.
pub struct ScriptedBackend {
    script: Vec<(Duration, BackendEvent)>,
}

impl ScriptedBackend {
    pub fn new(script: Vec<(Duration, BackendEvent)>) -> Self {
        Self { script }
    }
}

impl InputBackend for ScriptedBackend {
    fn name(&self) -> &'static str {
        "sher_input_test::ScriptedBackend"
    }

    fn spawn(self: Box<Self>, sink: BackendEventSink, running: Arc<AtomicBool>) -> Result<std::thread::JoinHandle<()>> {
        let script = self.script;
        std::thread::Builder::new()
            .name("sher-input-scripted-backend".to_string())
            .spawn(move || {
                for (delay, event) in script {
                    if !running.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(delay);
                    if !running.load(Ordering::SeqCst) {
                        return;
                    }
                    sink.send(event);
                }
                // Script exhausted: idle until asked to stop rather than exiting the
                // thread early, so a caller polling the JoinHandle doesn't observe a
                // backend that looks like it crashed (section 27).
                while running.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
            .map_err(|e| Error::Backend(e.to_string()))
    }
}
