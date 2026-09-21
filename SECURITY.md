# Security Policy

## Current status: early, partially real attack surface, not audited

Unlike a purely in-memory scaffold, SHER-Input already has a real attack
surface on Linux: `sher_input_linux::LinuxBackend` reads raw `evdev` input
events from `/dev/input` and parses them (`crates/linux/src/linux/reader.rs`,
`keymap.rs`). That parsing has never been fuzzed and has never had a
third-party security review. Treat it accordingly:

- **Never had a third-party security audit.**
- **No fuzzing at the evdev boundary.** No `cargo-fuzz`/`proptest` setup
  exists anywhere in this repo. Raw byte parsing of `/dev/input` happens
  inside the external `evdev` crate, not in SHER-Input's own code, so a
  useful fuzz target would feed malformed `evdev::InputEvent` streams into
  `translate_batch`/`keymap::map_key` rather than parsing raw bytes
  directly — this does not exist yet (see `ARCHITECTURE.md`, "What's
  intentionally not here yet").
- **No process-level isolation for the Linux backend.**
  `LinuxBackend::spawn` runs on a plain in-process OS thread — no seccomp
  filter, no namespace, no subprocess boundary between the code parsing
  untrusted device data and the rest of the process. A malicious or
  malformed HID device could exploit a bug in `evdev` or this crate's
  translation logic with full process privileges, not a sandboxed
  subprocess.
- **`cargo audit` has not been run against this repository's dependency
  tree in this pass** — the environment used for this review had no
  network access to fetch the RustSec advisory database. A dependency
  audit CI job has been added (see `.github/workflows/ci.yml`), but its
  first real run has not been observed to pass or fail yet.
- **The synthetic-input grant model enforces scope, not intent.**
  `SyntheticInputGrant` restricts a caller to one origin and one event kind
  (see `ARCHITECTURE.md`, "Synthetic input") — it stops a keyboard-only
  grant from moving the pointer, but SHER-Input itself has no opinion on
  *whether* a given grant should have been issued in the first place; that
  policy decision belongs entirely to whatever issues grants (not yet
  implemented — Phase 5 in `ARCHITECTURE.md`'s phase table).
- **Lock-poisoning is fail-fast, not fail-safe.** Internal state
  (`InputService`'s device/keyboard/pointer/touch/coalescer state,
  `CaptureRegistry`) is behind `std::sync::RwLock`/`Mutex` and every access
  uses `.expect("... lock poisoned")` (e.g. `crates/core/src/service.rs`,
  `crates/core/src/registry.rs`, `crates/core/src/capture.rs`). If any
  thread ever panics while holding one of these locks, every subsequent
  caller panics too instead of the service degrading gracefully. This is a
  deliberate choice (a poisoned lock means a real thread panic already
  broke an invariant), but it means one bug can escalate into a
  process-wide panic rather than being contained to the backend thread
  that failed.

Do not deploy this project in any context where its security properties
matter until this notice is updated.

## Reporting a vulnerability

If you find a security-relevant bug (memory-safety issue, evdev/HID parsing
bug, a way to bypass the capture or synthetic-input scoping, a lock-ordering
or poisoning issue that escalates a single-device failure into a
process-wide crash, or anything else that would matter once this code is
wired to real hardware), please report it privately rather than opening a
public GitHub issue:

- Email: mullassery@gmail.com
- Include: affected crate/file, a minimal reproduction if possible, and the
  potential impact.

This is a single-maintainer project. There is no dedicated security team, no
bug bounty, and no guaranteed response-time SLA — but reports will be read
and acknowledged.

## Supported versions

Only the latest tagged release and `main` are supported. See
[CHANGELOG.md](CHANGELOG.md) for release history.
