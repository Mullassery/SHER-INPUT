# SHER Input

[![Grapify](https://img.shields.io/badge/Grapify-Code%20Scan-purple)](https://grapify.ai)

[![CI](https://github.com/Mullassery/SHER-INPUT/actions/workflows/ci.yml/badge.svg)](https://github.com/Mullassery/SHER-INPUT/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

The input subsystem of the SHER platform: it turns physical device activity —
keyboards, mice, touchscreens, tablets, game controllers — into a single canonical,
ordered, timestamped event stream, and hands it to SHER-Display without ever letting
Linux keycodes, evdev paths, or HID reports leak past the boundary.

Most desktop stacks answer "who owns input" by letting the compositor reach straight
into `/dev/input`. That works until you want a second backend, a security boundary
around synthetic input, or a test suite that doesn't need real hardware. SHER-Input
is built as the layer that makes those things possible from day one, not bolted on
later: normalization, device lifecycle, state tracking, and event ordering live here,
once, independent of what's underneath.

```
Physical Device
      │
SHER-Kernel / Driver Layer
      │
SHER-Input                         <- this repository
      │   device discovery · normalization · state tracking
      │   sequencing · coalescing · capture contract
      ▼
SHER-Display        focus · pointer target · window routing
      │
      ▼
Aurora / Applications
```

Full design rationale — layering, the canonical event model, ordering and coalescing
guarantees, the capture and synthetic-input contracts — lives in
[`ARCHITECTURE.md`](ARCHITECTURE.md).

| | |
|---|---|
| **Phase** | 1 of 5 (device discovery, normalization, sequencing, capture/synthetic-input contracts) |
| **Real backend** | Linux/evdev — keyboard, pointer, scroll, hotplug |
| **Not yet real** | touch, tablet, gamepad (types exist, Phase 4 deferral) |
| **Tests** | 54 on Linux (36 platform-independent + 18 Linux-only), 36 elsewhere |
| **License** | Apache 2.0 |

## Contents

- [Use cases](#use-cases)
- [What's actually implemented](#whats-actually-implemented)
- [See it run](#see-it-run)
- [Building](#building)
- [Linux / Ubuntu compatibility](#linux--ubuntu-compatibility)
- [Repository layout](#repository-layout)
- [Architectural boundary](#architectural-boundary)
- [License](#license)

## Use cases

- **Building an input pipeline with a real capture/synthetic-input
  security model** — every grant scoped to one origin and one event kind,
  not an unrestricted hook into `/dev/input`.
- **Testing input-dependent code without real hardware** —
  `sher-input-monitor --simulate` and `sher_input_test::SimulatedController`
  drive the real pipeline deterministically.
- **Not yet a good fit for:** touch, tablet, or gamepad input (types
  exist, no backend yet — deliberate Phase 4 deferral); anything needing
  low-latency hotplug detection (currently polls `/dev/input` every 500ms,
  not `inotify`-based).

## What's actually implemented

This is Phase 1 of a five-phase plan, and the repository says so honestly rather than
aspirationally:

- A real Linux/evdev backend: device discovery, keyboard, relative pointer motion,
  buttons, low- and high-resolution scrolling, hotplug.
- A canonical event model covering keyboard, pointer, touch, tablet, and gamepad from
  day one — so later phases add backend logic, not breaking API changes.
- Per-device keyboard/pointer/touch state tracking, with physical-key / logical-key /
  text kept as three distinct things, never collapsed into one.
- Global sequencing and timestamps, so "A happened before B" is always answerable
  across devices — with controlled coalescing of high-frequency motion/scroll that
  never reorders or drops a button or key transition.
- An explicit, revocable capture contract for SHER-Display (drag, resize, pointer
  lock) and a synthetic-input model (accessibility tools, automated testing, remote
  desktop, AI agents) where every grant is scoped to one origin and one event kind —
  there is no unrestricted grant.
- Detectable, hot-restartable backend crashes (external critique, verified real gap
  — was previously an unverified design claim, not fixed code): `ARCHITECTURE.md`
  section 27 already asserted "a backend thread panicking ... does not take
  `InputService` down," true by construction (`InputBackend::spawn` runs on a real
  `std::thread`, which Rust itself contains a panic within), but nothing let a caller
  find out a backend had crashed, and there was no restart path. `BackendHandle`
  gained `is_finished()` (non-blocking liveness poll) and
  `stop_and_check_panicked()` (distinguishes a crash from a clean stop); a caller
  detects a dead backend and calls `InputService::start_backend` again with a fresh
  instance to hot-restart it — proved end to end with a backend that panics on
  purpose, an assertion that `InputService` keeps delivering events from other
  sources afterward, and a fresh backend instance resuming real event delivery on
  the same service (`crates/core/tests/backend_crash_recovery.rs`, plus
  `backend.rs`'s own unit tests for the panic/clean-stop distinction).
- 48 tests: 36 platform-independent (unit tests for state tracking/coalescing, plus
  integration tests that drive a simulated backend through the real `InputService`
  pipeline into a mock display-style router, proving the focus/capture boundary end to
  end) plus 12 that only compile and run on Linux — real synthetic `evdev` events
  (constructed with `evdev::InputEvent::new`, the same struct the kernel hands the
  driver) run through the real translation logic in `crates/linux/src/linux/{reader,
  keymap}.rs`, not a mock.

**Verified on real Linux, not just claimed:** `crates/linux`'s evdev backend has been
compiled and its full test suite run inside an actual Linux container (`rust:latest`
on `linux/arm64` via Docker), not only cross-compiled or assumed correct from macOS.
That first real compile caught two bugs that had shipped unnoticed because the crate
had only ever built on macOS, where `#[cfg(target_os = "linux")]` compiles it out
entirely: `map_button` referenced `evdev::Key::BTN_MISC`/`BTN_JOYSTICK`, which don't
exist in the `evdev` crate (fixed by reproducing the kernel header's range boundary as
raw codes), and a `PointerButton::Other(u8)` couldn't hold the evdev codes it was
meant to carry without silent truncation (widened to `u16`). A closer read of the same
code, prompted by the same exercise, found a third bug that Linux compilation alone
wouldn't have caught: motion (`REL_X`/`REL_Y`) was only flushed to the sink on
`SYN_REPORT`, so a button or scroll event landing in the same `fetch_events()` batch
*before* the terminating `SYN_REPORT` was sent to `InputService` ahead of the motion
that physically preceded it — inverting event order. All three are fixed, covered by
the 12 new Linux-only tests, and CI now builds/tests/clippies/fmt-checks on both
`ubuntu-latest` and `macos-latest` on every push, so a regression here fails loudly
instead of shipping unnoticed again.

**Not yet real:** touch, tablet, and gamepad have types but no backend — a deliberate
Phase 4 deferral, not an oversight. Hotplug topology discovery still polls `/dev/input`
every 500ms rather than using `inotify`. SHER-Display integration is proved against a
mock router in this repo's own tests; separately, SHER-Display's own crate
(`sher_display_input`) has real code consuming `sher_input_core::InputService` and
`sher_input_test::SimulatedController`, verified (5/5 tests) to still build and pass
against this repo's current API — see the phase table in `ARCHITECTURE.md` for the
rest.

## See it run

```
cargo run -p sher-input-monitor -- --simulate
```

drives a scripted demo keyboard and mouse through the real normalization,
sequencing, and coalescing pipeline — no hardware required:

```
sher-input-monitor: listening for normalized SHER-Input events (Ctrl+C to exit)

#1      [physical] device-added   1a698faf  "Demo Keyboard"  class=Keyboard caps(keys=true ...)
#2      [physical] device-added   cdcb4fb0  "Demo Mouse"     class=Pointer  caps(ptr_rel=true scroll=true ...)
#3      [physical] key-down 1a698faf  physical=KeyH logical=Character('h') text=Some("h")  mods=none
#4      [physical] key-up   1a698faf  physical=KeyH logical=Character('h') text=None
#7      [physical] motion-rel     cdcb4fb0  dx=12.0 dy=-4.0
#8      [physical] button-down    cdcb4fb0  Left
```

Drop `--simulate` on Linux with `/dev/input` access to see real devices flow through
the same pipeline, unchanged.

## Building

```
cargo build --workspace
cargo test --workspace
```

On any non-Linux host the Linux backend compiles to a stub returning
`Error::Unsupported`; `sher_input_test::ScriptedBackend` and `SimulatedController` are
what the test suite and `sher-input-monitor --simulate` use instead, and are meant to
be reused by anything downstream that needs deterministic input without hardware.

To build and test the real Linux backend from a non-Linux host, run it inside a Linux
container — this is exactly what CI does on `ubuntu-latest`, and how the backend was
first verified during development:

```
docker run --rm -v "$PWD":/work -w /work rust:latest \
  bash -c "cargo build --workspace && cargo test --workspace && \
           rustup component add clippy rustfmt && \
           cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check"
```

## Linux / Ubuntu compatibility

Verified 2026-10 (see org-wide `SHER-LINUX-RUST-COMPATIBILITY.md`):

- **Ubuntu**: 24.04 LTS and 26.04 LTS. Not just `cargo check` — the real test suite
  (54 tests, including the Linux backend) was run in real Ubuntu Docker containers on
  both arm64 (native) and x86_64 (emulated), as both root and a created non-root user.
- **Architecture**: x86_64 and arm64, both confirmed. All wire-format/endianness handling
  for `input_event` is delegated entirely to the `evdev` crate — this repo's own code has
  zero manual struct-layout or endianness logic.
- **Permissions**: a non-privileged user missing `input`-group/udev membership will now get
  a clear warning naming the inaccessible device/directory (fixed this pass — previously
  silent). A udev rule (`udev/70-sher-input.rules`) is shipped to grant the right access
  automatically on systemd-based Ubuntu installs.
- **Known limitations**: 500ms polling for new `/dev/input` nodes instead of inotify
  (disclosed Phase-1 limitation, tracked in `ROADMAP_HONEST.md`); no dedicated ARM64 CI
  runner yet (tested manually this pass, not yet automated in CI).

## Repository layout

```
crates/
  core/          sher_input_core   — canonical types, InputService, backend contract
  linux/         sher_input_linux  — the Linux/evdev backend
  test-support/  sher_input_test   — simulated backend + fixtures
tools/
  sher-input-monitor/  live diagnostic CLI — normalized events only, never
                        backend-specific detail
tests/           cross-crate contract tests
```

Three crates, not nine. `sher_input_core` covers every device class's types from day
one; a crate boundary exists only where it isolates something real — Linux-specific
code, or test fixtures — not one per device class for its own sake.

## The SHER family

SHER-Input is one repo in a multi-repo platform. For discoverability (including by
anyone or anything landing on just this repo), here are its siblings:

- [**SHER-KERNEL**](https://github.com/Mullassery/SHER-KERNEL) — the foundation: capability-based security, scheduling, memory, and driver-runtime object model the rest of SHER builds on.
- [**SHER-Process-Explorer**](https://github.com/Mullassery/SHER-Process-Explorer) — Linux process telemetry (`/proc`, perf, eBPF, cgroups, namespaces, containers).
- [**SHER-Graphics**](https://github.com/Mullassery/SHER-Graphics) — graphics architecture: pure-Rust software GPU/reference driver, capability-secured runtime, Vulkan/ash FFI backend.
- [**SHER-Display**](https://github.com/Mullassery/SHER-Display) — display server/compositor/window manager, with Wayland/X11 compatibility; the one sibling that depends on this repo (see below).
- [**SHER-Aurora**](https://github.com/Mullassery/SHER-Aurora) — GNOME/GTK4-oriented Rust design system.

Cargo-level coupling is narrower than this list — see "Architectural boundary" and
"Cross-repo compatibility" below for exactly which of these this repo actually depends
on at build time.

## Architectural boundary

SHER-Input does not modify — or currently even depend on — SHER-Kernel, SHER-Graphics,
or SHER-Display. Its canonical types (`InputDeviceId`, `Error`, ...) are intentionally
self-contained rather than reusing SHER-Kernel's `ObjectId`/`Error`, since exposing
another subsystem's types through SHER-Input's public API would itself be a boundary
violation. The contract with SHER-Display is proven with a mock router in `tests/`,
not by reaching into SHER-Display's own crate. A real dependency on SHER-Kernel (e.g.
for device handles from a future native backend) is expected eventually, but only gets
added when a concrete need exists — not speculatively.

### Cross-repo compatibility (verified, whole family)

This repo is one of 5 sibling repos under the Mullassery org: `SHER-Kernel`,
`SHER-Graphics`, `SHER-Display`, `SHER-Input` (this repo), and `Aurora`
(GitHub: `SHER-Aurora`). Of those, only `SHER-Display` actually depends on
this repo — via relative path (`../SHER-Input/crates/core`,
`../SHER-Input/crates/test-support`), consuming `sher_input_core` and
`sher_input_test`. Verified current: a from-scratch `cargo build
--workspace` in SHER-Display against this repo's present state compiles
clean, and `cargo test --workspace` there passes 56/56 — including
`sher_display_input`, which genuinely drives this repo's real
`sher_input_core::InputService` rather than a local reimplementation.
`SHER-Kernel`, `SHER-Graphics`, and `Aurora` have no Cargo-level dependency
on this repo in either direction.

## License

This project is licensed under the [Apache License 2.0](LICENSE).
