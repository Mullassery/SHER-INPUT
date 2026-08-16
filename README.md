# SHER Input

[![CI](https://github.com/Mullassery/SHER-INPUT/actions/workflows/ci.yml/badge.svg)](https://github.com/Mullassery/SHER-INPUT/actions/workflows/ci.yml)

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
- 42 tests: 30 platform-independent (unit tests for state tracking/coalescing, plus
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

## Architectural boundary

SHER-Input does not modify — or currently even depend on — SHER-Kernel, SHER-Graphics,
or SHER-Display. Its canonical types (`InputDeviceId`, `Error`, ...) are intentionally
self-contained rather than reusing SHER-Kernel's `ObjectId`/`Error`, since exposing
another subsystem's types through SHER-Input's public API would itself be a boundary
violation. The contract with SHER-Display is proven with a mock router in `tests/`,
not by reaching into SHER-Display's own crate. A real dependency on SHER-Kernel (e.g.
for device handles from a future native backend) is expected eventually, but only gets
added when a concrete need exists — not speculatively.

## License

See [`LICENSE`](LICENSE).
