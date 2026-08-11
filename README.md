# SHER Input

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
- 30 tests: unit tests for state tracking and coalescing, and integration tests that
  drive a simulated backend through the real `InputService` pipeline into a mock
  display-style router, proving the focus/capture boundary end to end.

**Not yet real:** the Linux backend has not been compiled or exercised on Linux —
this was built on macOS, so `crates/linux`'s Linux-only code sits behind
`#[cfg(target_os = "linux")]` and has never reached a compiler. Touch, tablet, and
gamepad have types but no backend. SHER-Display integration is proved against a mock
router in this repo, not the real SHER-Display crate. See the phase table in
`ARCHITECTURE.md` for the rest.

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

SHER-Input depends on `sher_common` (SHER-Kernel) as a read-only library dependency,
matching the layering above. It does not modify SHER-Kernel, SHER-Graphics, or
SHER-Display, and nothing in this repository does — the contract with SHER-Display is
proven with a mock router in `tests/`, not by reaching into SHER-Display's own crate.

## License

See [`LICENSE`](LICENSE).
