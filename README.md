# SHER Input

The input subsystem of the SHER family: it turns physical device activity — keyboards, mice, touchscreens, tablets, game controllers — into a single canonical, ordered, timestamped event stream, and hands it to SHER-Display without ever letting Linux keycodes, evdev paths, or HID reports leak past the boundary.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![CI](https://github.com/Mullassery/SHER-INPUT/actions/workflows/ci.yml/badge.svg)](https://github.com/Mullassery/SHER-INPUT/actions/workflows/ci.yml)

## What it is

SHER Input is the layer where normalization, device lifecycle, state tracking, and event ordering live — once, independent of what is underneath. It takes raw device activity and produces a canonical event model covering keyboard, pointer, touch, tablet, and gamepad, with global sequencing so "A happened before B" is always answerable across devices.

Most desktop stacks answer "who owns input" by letting the compositor reach straight into `/dev/input`. SHER Input is built as a separate layer so a second backend, a security boundary around synthetic input, or a hardware-free test suite are possible from day one rather than bolted on later.

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

## Features / Highlights

- **A real Linux/evdev backend** — device discovery, keyboard, relative pointer motion, buttons, low- and high-resolution scrolling, and hotplug.
- **A canonical event model from day one** covering keyboard, pointer, touch, tablet, and gamepad, so later phases add backend logic without breaking API changes.
- **Per-device state tracking** with physical-key / logical-key / text kept as three distinct things, never collapsed.
- **Global sequencing and timestamps** with controlled coalescing of high-frequency motion/scroll that never reorders or drops a button or key transition.
- **An explicit, revocable capture contract** for SHER-Display (drag, resize, pointer lock) and a synthetic-input model (accessibility tools, automated testing, remote desktop, AI agents) where every grant is scoped to one origin and one event kind — there is no unrestricted grant.
- **Detectable, hot-restartable backend crashes** — `BackendHandle::is_finished()` and `stop_and_check_panicked()` distinguish a crash from a clean stop, and `InputService::start_backend` can hot-restart with a fresh instance while other sources keep delivering events.
- **Reusable test harness** — `sher_input_test::ScriptedBackend`/`SimulatedController` drive the real pipeline deterministically with no hardware.

## Who it's for / Use cases

- **Building an input pipeline with a real capture/synthetic-input security model** — every grant is scoped to one origin and one event kind, not an unrestricted hook into `/dev/input`.
- **Testing input-dependent code without real hardware** — `sher-input-monitor --simulate` and `sher_input_test::SimulatedController` drive the real pipeline deterministically.
- **Feeding a compositor a backend-agnostic event stream** — SHER-Display's `sher_display_input` genuinely drives `sher_input_core::InputService` rather than reimplementing input handling.

## What it is not

- **Not feature-complete across device classes.** Touch, tablet, and gamepad have types but no backend yet — a deliberate Phase 4 deferral, not an oversight.
- **Not low-latency on hotplug.** New `/dev/input` nodes are currently detected by polling every 500 ms, not via `inotify` (a disclosed Phase-1 limitation).
- **Not coupled to SHER-Kernel today.** Canonical types such as `InputDeviceId` and `Error` are intentionally self-contained rather than reusing SHER-Kernel's `ObjectId`/`Error`; a real dependency is expected eventually but only when a concrete need exists.
- **Not a compositor or a display server.** Focus, pointer targeting, and window routing belong to SHER-Display; this repo stops at the canonical event stream and capture/synthetic contracts.
- **Not published as crates.** It is consumed by SHER-Display via a relative path dependency.

## Project status

**Phase 1 of 5** (device discovery, normalization, sequencing, capture/synthetic-input contracts).

| | |
|---|---|
| **Real backend** | Linux/evdev — keyboard, pointer, scroll, hotplug |
| **Not yet real** | touch, tablet, gamepad (types exist, Phase 4 deferral) |
| **Tests** | 54 on Linux (37 platform-independent + 17 Linux-only), 37 elsewhere |
| **Version** | 0.3.0 (edition 2021) |

- **Implemented and tested:** the evdev backend, canonical event model, per-device state tracking, sequencing/coalescing, capture and synthetic-input contracts, and backend crash detection/restart.
- **Verified on real Linux:** `crates/linux` has been compiled and tested inside real Ubuntu containers (arm64 native and x86_64 emulated, as root and a non-root user), not just cross-compiled. That pass caught and fixed three real bugs (two evdev compile issues and a motion-ordering inversion), now covered by Linux-only tests, with CI building/testing/clippy/fmt-checking on both `ubuntu-latest` and `macos-latest`.
- **Not started:** non-Linux backends, and backends for touch/tablet/gamepad.

## Installation

Not published to any registry. Build from source with a stable Rust toolchain (edition 2021):

```bash
git clone https://github.com/Mullassery/SHER-INPUT.git
cd SHER-Input
cargo build --workspace
```

On Linux with systemd, install the bundled udev rule so a non-privileged user in the `input` group gets access to the required devices:

```bash
sudo cp udev/70-sher-input.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Quick start

Drive a scripted demo keyboard and mouse through the real normalization, sequencing, and coalescing pipeline — no hardware required:

```bash
cargo run -p sher-input-monitor -- --simulate
```

```
sher-input-monitor: listening for normalized SHER-Input events (Ctrl+C to exit)

#1      [physical] device-added   1a698faf  "Demo Keyboard"  class=Keyboard caps(keys=true ...)
#2      [physical] device-added   cdcb4fb0  "Demo Mouse"     class=Pointer  caps(ptr_rel=true scroll=true ...)
#3      [physical] key-down 1a698faf  physical=KeyH logical=Character('h') text=Some("h")  mods=none
#7      [physical] motion-rel     cdcb4fb0  dx=12.0 dy=-4.0
```

Drop `--simulate` on Linux with `/dev/input` access to see real devices flow through the same pipeline, unchanged.

## Usage

On any non-Linux host the Linux backend compiles to a stub returning `Error::Unsupported`; `sher_input_test::ScriptedBackend` and `SimulatedController` are what the test suite and `sher-input-monitor --simulate` use instead, and are meant to be reused by anything downstream that needs deterministic input without hardware.

Build/test the real Linux backend from a non-Linux host inside a Linux container (exactly what CI does on `ubuntu-latest`):

```bash
docker run --rm -v "$PWD":/work -w /work rust:latest \
  bash -c "cargo build --workspace && cargo test --workspace && \
           rustup component add clippy rustfmt && \
           cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check"
```

Repository layout:

```
crates/
  core/          sher_input_core   — canonical types, InputService, backend contract
  linux/         sher_input_linux  — the Linux/evdev backend
  test-support/  sher_input_test   — simulated backend + fixtures
tools/
  sher-input-monitor/  live diagnostic CLI — normalized events only, never
                        backend-specific detail
tests/           cross-crate contract tests
udev/            udev rule for device access on systemd-based Linux
```

## Documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — design rationale: layering, the canonical event model, ordering/coalescing guarantees, and the capture and synthetic-input contracts
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development workflow and the architectural rules
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
- [`SECURITY.md`](SECURITY.md) — how to report a security issue
- [`CHANGELOG.md`](CHANGELOG.md)
- [`ROADMAP_HONEST.md`](ROADMAP_HONEST.md) — status and technical-debt audit

## Contributing

Contributions are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the development workflow and the architectural rules this project holds to (self-contained canonical types; no backend-specific detail leaking across the boundary), and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). This is a single-maintainer project, so review happens on a best-effort basis rather than a fixed schedule; report security issues per [`SECURITY.md`](SECURITY.md).

## Part of the SHER OS family

SHER-INPUT is part of the SHER OS family — an AI-native operating system built from scratch in Rust. The subsystems are designed to work together as a coherent OS stack.

| Tool | Role | What it does |
|---|---|---|
| [SHER-KERNEL](https://github.com/Mullassery/SHER-KERNEL) | Kernel core | Core kernel scaffolding: object model, scheduling, memory, driver-lifecycle concepts, and a Linux Kernel Interface (LKI) compatibility layer. |
| [SHER-Graphics](https://github.com/Mullassery/SHER-Graphics) | Graphics/GPU | GPU abstraction and rendering — software simulation plus a real, working Vulkan/MoltenVK backend. |
| [SHER-Display](https://github.com/Mullassery/SHER-Display) | Display/compositor | Display server, compositor, and window management. |
| [SHER-INPUT](https://github.com/Mullassery/SHER-INPUT) | Input | **This project** — canonical, backend-independent input event stream — real evdev on Linux, simulated backend for hardware-free testing. |
| [SHER-Aurora](https://github.com/Mullassery/SHER-Aurora) | Design system | GNOME-style design system — tokens, typography, motion, and an automated WCAG contrast audit. |
| [SHER-Process-Explorer](https://github.com/Mullassery/SHER-Process-Explorer) | Observability | Evidence-based Linux process investigation tool — /proc telemetry, bpftrace integration, daemon + CLI + desktop UI. |

Each works standalone; they're complementary by design. [Browse all projects →](https://github.com/Mullassery?tab=repositories)

## License

This project is licensed under the [Apache License 2.0](LICENSE).
