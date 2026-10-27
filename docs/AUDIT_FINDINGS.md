# Audit Findings: SHER-Input

**Audit Date:** 2026-10-26  
**Version:** 0.3.0  
**Branch:** `main`  
**Full report:** [`.audit/reports/SHER-Input/audit.md`](../.audit/reports/SHER-Input/audit.md)

## Scope

Read-only review of the input event model, `InputService`, Linux evdev backend, synthetic/capture contracts, tests, CI, udev rules, and documentation. No real `/dev/input` session was run.

## Findings

| ID | Severity | Finding | Evidence | Recommendation |
|---|---|---|---|---|
| SI-01 | High/Medium | Backend ingress queue is unbounded. | `InputService::new`: `mpsc::unbounded_channel()`. | Use a bounded queue with an explicit overflow policy. |
| SI-02 | Medium | Reader threads cannot be joined on shutdown. | `reader.rs` blocks in `fetch_events`; `BackendHandle` joins supervisor only. | Add interruptible reads and join all reader threads. |
| SI-03 | Medium | Broadcast lag can drop key/button events. | 4096-slot `broadcast`; no service-level `Lagged` policy. | Define resync/per-subscriber queues and correct the “never drops” claim. |
| SI-04 | Medium | No `EVIOCGRAB`; capture is app-level only. | No grab in Linux backend. | Document the boundary or implement exclusive capture. |
| SI-05 | Medium | Synthetic grants are caller-constructed with no origin authentication. | `SyntheticInputGrant` / `submit_synthetic`. | Authenticate origin at the boundary and log grants/denials. |
| SI-06 | Medium | No capture/synthetic audit trail. | No logging in capture/synthetic paths. | Emit structured security events. |
| SI-07 | Low/Medium | Device access depends on udev `uaccess`. | `udev/70-sher-input.rules`; group fallback commented. | Document/support non-udev environments. |
| SI-08 | Low | README test count and “joins it” wording are stale. | README 54 vs 58 static tests; supervisor-only join. | Update docs. |
| SI-09 | Low/Medium | No burst/slow-subscriber/shutdown stress tests. | CI review. | Add stress tests for queue depth, lag, and blocked readers. |
| SI-10 | Low | Hotplug latency is 500 ms. | Supervisor poll interval. | Keep documented; consider inotify/netlink later. |

## Positive Controls

- Canonical event model with physical/synthetic provenance.
- Global sequencing and coalescing that preserves key/button order.
- Real evdev backend plus deterministic simulated backend.
- Backend crash detection/hot restart.
- Scoped synthetic grants; hotplug events denied.
- Narrow `uaccess` udev rule.
- Good Linux/macOS CI and honest phased limitations.

## Audit Limitations

- No real device/udev session.
- Queue/lag/shutdown findings are static, not reproduced under load.
- `EVIOCGRAB` absence was found by source search.
- Test counts are definitions, not a verified pass count.
- No multi-process synthetic-input test.
