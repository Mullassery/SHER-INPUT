# Audit Report: SHER-Input

**Audit Date:** 2026-10-26  
**Repository Path:** `/Users/georgimullassery/Desktop/Projects/SHER-Input`  
**Audit Type:** Read-only systems design audit  
**Version:** 0.3.0 (workspace)  
**Branch:** `main`  
**Source Size:** ~4,892 Rust LOC; 5 workspace members; 58 tests

## Executive Summary

SHER-Input is the canonical input layer of the SHER stack: a backend-independent event model, per-device state tracking, sequencing and coalescing, capture/synthetic-input contracts, and a real Linux evdev backend. The design is thoughtful: raw device paths are hidden, physical and synthetic events are distinguishable, `SYN_REPORT` ordering is respected, motion is coalesced without reordering keys/buttons, and backend crashes are pollable/restartable.

The main issues are backpressure and shutdown. Reader threads push into an unbounded Tokio mpsc queue, so a burst or a stalled pump can grow memory without limit. Backend shutdown joins the supervisor thread but not the per-device reader threads; a reader parked in blocking `fetch_events()` survives until a device error or process exit. The 4096-slot broadcast fan-out can lag a slow subscriber, which contradicts the README’s broad “never drops a button or key transition” claim unless subscriber lag is explicitly handled upstream. The kernel device layer also does not use `EVIOCGRAB`, so “capture” is a display-level contract, not exclusive hardware capture.

## Purpose and Positioning

- **What it is:** a Rust input normalization/routing layer with a real evdev backend, deterministic simulated backend, monitoring CLI, and cross-crate contract tests.
- **Who it is for:** SHER-Display, Aurora, and other consumers that need a stable event stream independent of the underlying device API.
- **What it is not:** a compositor, an input-method framework, an accessibility implementation, a touch/tablet/gamepad backend, or a security sandbox for synthetic input.
- **Differentiator:** global sequencing plus per-device state and coalescing, with physical/synthetic provenance and backend crash isolation.

## Architecture

```text
Linux evdev (/dev/input/event*)
  supervisor thread (hotplug scan, ~500 ms)
  one blocking reader thread per device
        |
        v
BackendEvent (unbounded Tokio mpsc)
        |
        v
InputService pump task
  registry, keyboard/pointer/touch state
  coalescer (8 ms ticker + explicit flush)
  sequence authority
        |
        v
broadcast::Sender<InputEvent> (capacity 4096)
        |
        +--> SHER-Display / Aurora / monitor / tests

Synthetic path
  SyntheticInputGrant -> submit_synthetic -> same ingest path, tagged Synthetic
```

### Event ordering

```text
reader batch
  -> REL_X/REL_Y accumulated
  -> flush pending motion before every key/button/scroll/SYN
  -> BackendEvent
  -> InputService ingestion
  -> coalescer
  -> sequence assignment
  -> broadcast
```

## Repository Layout

```text
crates/core/                event model, service, registry, capture, config,
                            keyboard/pointer/touch state, coalescer, ring buffer
crates/linux/               evdev backend, hotplug supervisor, per-device readers
crates/test-support/        ScriptedBackend / SimulatedController
tools/sher-input-monitor/   live diagnostic CLI
tests/                      cross-crate contract tests
examples/                   ring-buffer bridge example
udev/                       device permission rule
.github/workflows/           CI + audit
```

## Technology Stack and Dependencies

- **Rust:** 2021 edition; Tokio, crossbeam-queue, evdev (Linux), serde, uuid, tracing.
- **Testing:** Rust unit/integration tests; simulated backend; Linux-only evdev tests.
- **CI:** Ubuntu + macOS; build/test/clippy/fmt; cargo audit.
- **Platform behavior:** real evdev on Linux; unsupported stub on non-Linux.
- **Device permissions:** udev `uaccess` tag for `event*` nodes.

## Public API

- `sher_input_core::InputService`: registry, subscribe, start/stop backend, capture requests, synthetic submission, coalesced flush.
- `InputEvent`, `InputEventPayload`, `InputSource`, `SequenceCounter`, `Timestamp`.
- Per-device keyboard/pointer/touch state and layout traits.
- `CaptureRegistry`, `CaptureGuard`, `CaptureKind`, `PointerGrabMode`.
- `SyntheticInputGrant`.
- `InputConfig`.
- `InputRingBuffer`/`RawInputEvent` lock-free SPSC helper.
- `sher_input_linux` backend.
- `sher_input_test::{ScriptedBackend, SimulatedController}`.
- `sher-input-monitor` CLI with simulate mode.

## Design Patterns

- **Backend boundary:** backends only emit `BackendEvent`; they never see each other or the display.
- **Provenance:** `InputSource::Physical` vs `Synthetic(origin)`.
- **Sequence authority:** one global counter makes cross-device ordering answerable.
- **Coalescing without reordering:** motion/scroll may merge; key/button transitions always flush pending motion first.
- **Weak self-reference:** the pump/ticker tasks hold `Weak<InputService>` to avoid an `Arc` cycle.
- **Capability-scoped synthetic input:** grants are per event kind, and device-management events are always denied.
- **Crash isolation:** backend handle can report panic vs clean stop and support hot restart.

## Data and Storage

- In-memory device registry and per-device state tables.
- In-memory capture registry and sequence counter.
- No database, persistence, credentials, or user data store.
- Backend ingress is an unbounded mpsc queue; subscriber fan-out is a 4096-slot broadcast channel.
- The SPSC ring buffer is available for high-rate handoff but is not the main service ingress path.
- No audit log for synthetic-input grants or capture requests.

## Concurrency and Performance

- One supervisor thread plus one blocking reader thread per device.
- The pump is a Tokio task; coalescing flush has an independent 8 ms ticker.
- **Unbounded ingress:** `mpsc::unbounded_channel()` means no backpressure from reader threads to the pump.
- **Reader shutdown:** `running` is checked only between `fetch_events()` batches; an idle device can park forever. `BackendHandle::drop` joins the supervisor, not reader threads.
- **Fan-out loss:** `broadcast::channel(4096)` can drop old events for a slow receiver (`Lagged`). The service sends and ignores errors when there are no receivers.
- **Locking:** state tables and coalescer use mutexes; a poisoned lock panics on `expect`.
- **Hotplug latency:** 500 ms polling interval.
- **Ring buffer:** SPSC, power-of-two capacity, one slot unused; suitable for ISR-style handoff but not used by `InputService`.
- No benchmarks for event rate, coalescing overhead, or subscriber lag.

## Security and Privacy

### Strengths

- Raw `/dev/input` paths are not exposed in the event model.
- Physical vs synthetic provenance is explicit.
- Synthetic input requires a scoped grant and is denied for hotplug/tablet/gamepad events.
- Capture requests are explicit and guard-based.
- Backend crash is isolated from `InputService`.
- udev rule uses `TAG+="uaccess"` rather than broad group access.
- No network, credentials, or telemetry.

### Gaps

- **No exclusive kernel grab:** no `EVIOCGRAB`; other processes can still read the same device. “Capture” is a display/application contract, not exclusive hardware ownership.
- **Synthetic trust boundary:** a caller that can construct a `SyntheticInputGrant` can inject events; there is no independent authentication of the origin.
- **No audit trail:** capture/synthetic grants and denials are not logged as security events.
- **Device permissions depend on udev:** the `uaccess` rule covers `event*` only; the group fallback is commented out, so non-udev/non-systemd environments may fail.
- **Unbounded queue:** an untrusted/high-rate backend can exhaust memory.
- **No per-event size limit** (payload is fixed-size, but queue depth is unbounded).
- **No encryption/privacy concerns** for input data beyond the OS device permissions.

## Reliability and Observability

- Backend panic detection via `is_finished` / `stop_and_check_panicked`.
- Reader errors are treated as unplugged and emit `DeviceRemoved`.
- Hotplug scanning adds new devices without retrying failed ones.
- Coalescer ticker guarantees idle motion eventually flushes.
- No metrics/tracing for queue depth, dropped/lagged subscribers, event rate, or reader state.
- No bounded ingress or load shedding.
- No clean shutdown for parked reader threads.
- No subscriber `Lagged` recovery policy centralized in the service.
- No health endpoint.

## Testing, CI, and Quality

- 58 static tests; README says 54 (37 platform-independent + 17 Linux-only).
- Tests cover sequencing/coalescing, ordering around keys/buttons, per-device state, capture/synthetic scoping, backend crash/restart, ring buffer, and Linux translation.
- Simulated backend makes most behavior hardware-free and deterministic.
- CI runs Ubuntu + macOS, build/test/clippy/fmt, and cargo audit.
- No stress test for queue growth, slow subscribers, or many devices.
- No test for reader-thread shutdown while blocked.
- No test for `EVIOCGRAB`/exclusive capture because it is not implemented.
- No fuzzing for evdev translation or ring-buffer concurrency.
- No coverage gate.

## Documentation, Packaging, and Release

- Workspace version is 0.3.0.
- README accurately documents real Linux validation, simulated backends, and non-Linux stubs.
- README says 54 tests; static count is 58.
- ARCHITECTURE.md explicitly documents the `EV_ABS` drop and 500 ms hotplug behavior.
- README says a backend handle “joins it”; that is true for the supervisor thread, not the per-device readers.
- The “never drops a button or key transition” claim is true for coalescing, but not automatically true for broadcast lag.
- No published package or release provenance.
- No tagged-release audit.

## Strengths

- Clear, minimal canonical event model with provenance.
- Strong ordering/coalescing discipline and tests.
- Real evdev backend plus deterministic simulated backend.
- Backend crash isolation and hot-restart support.
- Scoped synthetic-input grants; hotplug spoofing denied.
- Narrow udev `uaccess` rule.
- Good Linux/macOS CI coverage and clean separation of platform code.
- Honest documentation of phased limitations (`EV_ABS`, touch/tablet/gamepad).

## Risks and Gaps

| Risk | Severity | Evidence | Impact |
|---|---|---|---|
| Unbounded backend ingress queue | High/Medium | `InputService::new`: `mpsc::unbounded_channel()` | Memory exhaustion under bursts or a stalled pump. |
| Reader threads cannot be joined on shutdown | Medium | `reader.rs` blocks in `fetch_events`; `BackendHandle` joins supervisor only | Backend restart/shutdown leaks threads and open devices. |
| Broadcast lag can drop key/button events | Medium | 4096-slot `broadcast`; service does not handle `Lagged` | Slow subscribers can lose transitions despite the “never drops” claim. |
| No exclusive device capture | Medium | No `EVIOCGRAB` observed | Other processes can read input; capture is only an app contract. |
| Synthetic grants are caller-constructed | Medium | `SyntheticInputGrant`; `submit_synthetic` | Any in-process caller with a grant can inject; no origin authentication. |
| No capture/synthetic audit trail | Medium | No logging in capture/synthetic paths | Abuse/denials are hard to investigate. |
| udev dependency for device access | Low/Medium | `70-sher-input.rules`; group fallback commented | Non-udev systems may have no usable devices. |
| Test-count/docs drift | Low | README 54 vs 58 static tests; “joins it” wording | Minor release-truth issue. |
| No queue/subscriber stress tests | Low/Medium | CI review | Burst/lag behavior is unproven. |
| Hotplug latency is 500 ms | Low (known) | supervisor poll interval | Slow device discovery. |

## Prioritized Recommendations

### P0 — backpressure and shutdown

1. Replace the unbounded backend queue with a bounded queue and an explicit overflow policy (coalesce, drop motion-only, or fail/stop the backend). Never drop key/button transitions silently.
2. Give each reader an interruptible wait (`epoll`/`mio`, eventfd, or device close) so shutdown can join reader threads. Track all reader handles in the backend handle and join them.
3. Define a subscriber-lag contract: either propagate `Lagged` to consumers with a resync mechanism or use per-subscriber queues with a documented policy. Reconcile the README’s “never drops” claim.

### P1 — security and truth

4. Decide whether “capture” needs exclusive kernel input (`EVIOCGRAB`). If not, document that other processes can still read devices.
5. Authenticate synthetic-input origins at the boundary (capability handle/process identity), and add audit events for grants/denials.
6. Add metrics/tracing for queue depth, lagged receivers, dropped/coalesced counts, reader state, and hotplug events.
7. Update README test count and clarify that `BackendHandle` joins the supervisor, not readers.

### P2 — verification

8. Add stress tests for bursts, slow subscribers, many devices, and shutdown while a reader is blocked.
9. Add fuzz/property tests for evdev translation and SPSC ring concurrency.
10. Add a coverage gate and a benchmark for event throughput/coalescing.
11. Clarify the udev fallback for non-systemd environments.

## Audit Limitations

- No real `/dev/input` hardware session or udev installation test was run.
- Queue-growth, lag, and shutdown findings were identified by static inspection, not reproduced under load.
- `EVIOCGRAB` absence was established by source search, not by strace.
- Test counts are static definitions, not a verified pass count.
- Synthetic-input trust was assessed at the library boundary, not in a multi-process deployment.
