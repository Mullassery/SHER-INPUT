# Changelog

All notable changes to this project are documented in this file. Format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

No version history is fabricated here — every entry corresponds to a real
commit; see `git log` for the full commit-by-commit history.

## [Unreleased]

Changes on `main` since the `v0.2.0` tag (`Cargo.toml` is at `0.3.0` but this
has not been tagged as a release yet):

### Fixed
- `InputService::submit_synthetic` no longer accepts synthetic
  `DeviceAdded`/`DeviceRemoved` events regardless of grant scope.
  `SyntheticInputGrant` has no field for device management (only
  keyboard/pointer/touch), so a grant scoped to e.g. keyboard-only input
  could previously still spoof a device into (or out of) the registry via
  the synthetic-input path — contradicting the documented "no
  `SyntheticInputGrant::unrestricted()`" invariant. Physical hotplug is
  unaffected. See `ROADMAP_HONEST.md` item 9.

### Added
- `BackendHandle::is_finished()` (non-blocking liveness poll) and
  `stop_and_check_panicked()` (distinguishes a backend crash from a clean
  stop), so a caller can detect a dead backend and hot-restart it by calling
  `InputService::start_backend` again — proved end to end in
  `crates/core/tests/backend_crash_recovery.rs` with a backend that panics
  on purpose.
- Documented, verified cross-repo compatibility with `SHER-Display`
  (relative-path dependency on `sher_input_core`/`sher_input_test`; a
  from-scratch build there passes 56/56 tests against this repo's current
  API).
- "Use cases" section in README.md, including an explicit "not yet a good
  fit for" list (touch/tablet/gamepad, low-latency hotplug).

### Changed
- Relicensed from a prior license to Apache License 2.0.
- README now describes the license terms inline rather than only pointing
  at `LICENSE`.

### Documentation
- Added known-gaps section to `ARCHITECTURE.md` from an external critique
  review: no driver-level sandboxing (`LinuxBackend::spawn` uses plain
  in-process OS threads, no seccomp/namespace/subprocess isolation), no
  hot-restart of a dead device reader thread (only new-device polling
  existed at the time), no fuzzing at the SHER-Input/evdev boundary.

## [0.2.0]

- Bumped to 0.2.0 and documented real Linux verification in the README:
  `sher_input_linux`'s evdev backend compiled and its full test suite run
  inside an actual Linux container, not only cross-compiled or assumed
  correct from macOS.
- Fixed three real bugs found by that first real Linux compile:
  `map_button` referenced nonexistent `evdev::Key` constants
  (`BTN_MISC`/`BTN_JOYSTICK`), `PointerButton::Other(u8)` silently truncated
  evdev codes that didn't fit in a `u8` (widened to `u16`), and relative
  motion (`REL_X`/`REL_Y`) was only flushed on `SYN_REPORT`, which could
  deliver a same-batch button/scroll event to `InputService` ahead of
  motion that physically preceded it (inverting event order).
- Added CI: build/test/clippy/fmt on `ubuntu-latest` and `macos-latest`.
- Applied `cargo fmt` across the codebase.
- Removed an unused `sher_common` dependency.

## [0.1.0] — Phase 1 initial scaffold

- Canonical event model (`InputEvent`, keyboard/pointer/touch/tablet/gamepad
  payloads, `Physical`/`Synthetic` source tagging).
- `InputService` orchestrator: device registry, per-device keyboard/pointer/
  touch state tracking, global sequencing, motion/scroll coalescing with an
  8ms flush ticker, capture contract (`request_capture`, RAII revocation),
  synthetic-input grants scoped to origin + event kind.
- `sher_input_linux`: real Linux/evdev backend (device discovery, keyboard,
  relative pointer motion, buttons, scrolling, hotplug via 500ms polling);
  compiles to an `Error::Unsupported` stub on non-Linux hosts.
- `sher_input_test`: `ScriptedBackend` and `SimulatedController` for
  deterministic testing without hardware.
- `sher-input-monitor`: diagnostic CLI, `--simulate` mode.
- Initial test suite and `ARCHITECTURE.md`.

### Known incomplete / not started (see [ROADMAP_HONEST.md](ROADMAP_HONEST.md))
Touch, tablet, and gamepad backends (types exist, no implementation — Phase
4); SHER-Display focus/pointer-targeting/capture integration (Phase 2 — this
repo's own tests prove the contract against a mock router only); Aurora
integration (Phase 3); deeper synthetic-input policy beyond kind-scoping
(Phase 5); `inotify`-based hotplug; driver-level sandboxing; fuzzing at the
evdev boundary.
