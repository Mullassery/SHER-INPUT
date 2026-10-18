# Audit findings — 2026-10-19

This file records the repository-specific findings from the portfolio audit
conducted on 2026-10-19. It is documentation only; no code or release artifacts
were changed in this pass unless explicitly noted under "Implementation status".

Sources: the repository's own README/source/workflows, GitHub metadata and
Actions runs, PyPI metadata, and official competitor documentation. Claims are
marked as verified, inferred, or open questions. "Absent" means not found in the
inspected repository/docs, not proof that it cannot exist.

## What it is

Canonical input subsystem for SHER: Linux evdev backend, event normalization/sequencing, capture/synthetic-input contracts, and crash recovery.

## Verified strengths

- Real Linux evdev backend; canonical model; per-device state; sequencing/coalescing; capture/synthetic contracts; backend crash detection/restart; 54 tests; verified in Ubuntu containers.
- Real backend + reusable test harness.

## Verified gaps and user-facing risks

- Touch/tablet/gamepad types but no backend; 500 ms polling hotplug; not coupled to SHER-Kernel; no crates.io.

## Direct competitors

- libinput, SDL3, evdev.

## Ranked next actions

- P3: freeze; add device classes only if a SHER consumer needs them.

## Do not build

- Non-Linux backends and more device classes; no demand.

## Evidence gaps / validation needed

- No real desktop consumer; no latency benchmark.

## Recommended disposition

Freeze as part of the SHER research portfolio.

## Implementation status

No code changes in this pass.
