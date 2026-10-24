# Competitive intelligence in roadmap and upgrade work

**Policy date:** 2026-10-19

Competitive findings are an input to product decisions, not a one-off report.
Every pending roadmap item, new feature, and routine dependency upgrade should
check the repository's [`AUDIT_FINDINGS.md`](AUDIT_FINDINGS.md) before work
starts, and record the decision in the PR or changelog.

## Required check before building

1. **Read `AUDIT_FINDINGS.md`.** Find the relevant competitor row and the
   ranked next actions. If the finding is stale, update it with a dated source.
2. **Name the competitor and the user problem.** A feature is not justified by
   "competitor has it". State the workflow it unlocks, who needs it, and how
   often. Mark unverified demand as a hypothesis.
3. **Prefer reuse over new code.** Check whether an existing Mullassery repo
   already provides the capability (see `BUNDLES.md` in the profile repo and
   the bundle role file in this repo, when present). Typical reuse:
   - crawling/fetching/robots/SSRF → **MudFish**
   - architecture maps / PR diffs → **CodeAtlas**
   - data-quality contracts / validation → **StatGuardian**
   - token counting / cost tables → **PyTokenCalc**
   - dependency graph / advisories / SBOM → **PyDeepMesh**
   - segmentation / CLV / churn → **ClusterAudienceKit**
   - activation / lineage / PII → **PyReverseETL**
   - fleet state sync → **SyncFleet** (only after conflict resolution)
4. **Choose one disposition:** build, integrate, defer, or reject. Write it
   down. A rejection with a reason is a successful outcome.
5. **For routine upgrades**, check whether the upgrade closes a known
   competitor gap or breaks a differentiated capability. Run the tests that
   cover the affected path; do not merge a green build that silently changes a
   user-visible contract.

## PR / changelog checklist

- [ ] Competitor and source URL/date recorded (or "no direct competitor").
- [ ] User impact and frequency stated; unverified demand labelled.
- [ ] Reuse check done against sibling repos/bundles.
- [ ] Failure mode and rollback considered (especially auth, PII, cost).
- [ ] Docs/roadmap status updated honestly.
- [ ] Tests or a manual verification command included.

## Review cadence

- Refresh the competitor row when a release changes the capability.
- Re-check the full `AUDIT_FINDINGS.md` at least quarterly and before a major
  release.
- If a finding is wrong or stale, correct it in the same PR that relies on it.

## Non-goals

- Copying features for parity.
- Adding a dependency just because a competitor uses it.
- Treating star counts or download counts as proof of user need.

## OSS gap analysis — 2026-10-24

**What matters most here:** absolute devices and real pointer configuration. Dropping `EV_ABS` means no touch or tablet events, and `PointerConfig` is inert; fix absolute axes and wire configuration before claiming a canonical input layer.

Scope: workspace v0.3.0 (`sher_input_core`, `sher_input_linux`,
`sher_input_test`, `sher-input-monitor`). Phase 1 only: Linux evdev keyboard,
relative pointer, scroll, 500 ms polling hotplug, sequencing/coalescing, capture
and synthetic-input contracts. Touch/tablet/gamepad are types without backends.

### Comparable OSS projects

| Project | GitHub | License | Last activity | Why it is the comparison |
|---|---|---|---|---|
| SDL3 | https://github.com/libsdl-org/SDL | Zlib | 3.4.18 released 2026-09-02; commits 2026-10-24 | Cross-platform input subsystem (keyboard, pointer, touch, gamepad, hotplug) used by games and emulators |
| Smithay | https://github.com/Smithay/smithay | MIT | v0.7.0 tagged 2025-06-24; commits 2026-10-20 | Rust Wayland compositor library with libinput, seat, and input handling; the active Rust analog to wlroots input |
| evdev | https://github.com/emberian/evdev | Apache-2.0 OR MIT | v0.13.2; commits 2026-05-28 | Rust reimplementation of libevdev and the raw layer our Linux backend builds on |

libinput is the closest functional peer, but its canonical repository is GitLab
(MIT, 1.32.0 released 2026-09-17) and the GitHub mirrors are stale, so it is not tabled.

### Capability comparison

| Capability | Ours | SDL3 | Smithay | evdev | Gap |
|---|---|---|---|---|---|
| Touch and tablet support | Partial | Yes | Partial | Partial | Weaker: types exist, but EV_ABS is dropped so no events are produced |
| Hotplug detection | Partial | Yes | Yes | No | Weaker: 500 ms polling versus event-driven discovery |
| Layout mapping and text separation | Partial | Yes | Yes | No | Weaker: US QWERTY only, with a `set_layout` hook |
| Pointer acceleration, natural scroll, tap-to-click | No | Partial | Yes | No | Absent: config fields are never read outside `config.rs` |
| Per-device state tracking | Yes | Partial | Partial | Partial | Ahead: keyboard/pointer/touch state tables per device |
| Global sequencing and timestamps | Yes | Partial | Partial | No | Ahead: monotonic global sequence, not per-device timestamps |
| Synthetic-input scoping | Yes | Partial | Partial | Partial | Differentiated: origin + event-kind grants, Physical/Synthetic tagged |
| Backend crash detection and restart | Yes | No | Partial | No | Ahead narrowly: whole-backend panic detection and restart |
| Multi-seat / session handoff | No | Partial | Yes | No | Absent: direct `/dev/input` access, no libseat/logind |

### Gaps that matter (ranked)

1. **Absolute-axis devices produce no events (absent, M).** `translate_batch`
   discards `EV_ABS`, so touchscreens and tablets never reach the `Touch`/`Tablet`
   payloads; `device.rs` detects the axes but the reader has no path. This
   contradicts the "canonical model from day one" framing.
2. **Pointer configuration is inert (absent, S/M).** `PointerConfig`/`TouchConfig`
   expose speed, acceleration, natural scrolling, left-handed, tap-to-click, and
   palm rejection, but nothing outside `config.rs` reads them; the config surface
   promises behavior the normalizer does not implement.
3. **Hotplug is a 500 ms poll (weaker, S/M).** `POLL_INTERVAL`
   (`linux/src/linux/backend.rs:9`) leaves new devices unseen for up to half a
   second; there is no udev/inotify or property handling, while SDL3 and Smithay
   are event-driven.
4. **No seat or session management (absent, M/L).** The backend opens
   `/dev/input/eventN` under Unix permissions with no libseat/logind negotiation,
   multi-seat support, or release on VT switch; Smithay is built around this.
5. **Reader-thread death is invisible (weaker, S).** `supervise()` only checks
   `path.exists()`, so a panicked per-device reader leaves that device silently dead
   until replug; whole-backend recovery does not cover this narrower case.
6. **Untrusted evdev parsing is unfuzzed and unsandboxed (absent, M).**
   `keymap.rs`/`translate_batch` parse kernel data in-process with no fuzz target and
   no seccomp/subprocess isolation; the repository's own roadmap calls this real work.

### Where we are differentiated

- Physical key, logical key, and text are three distinct fields, so shortcut
  matching and text input do not reconstruct each other; SDL3/evdev expose
  scancode/keycode or raw events, and Smithay relies on xkbcommon.
- Global sequencing plus the coalescing invariant (a button transition flushes
  pending motion; an 8 ms ticker drains idle samples) is a stronger ordering
  contract than the per-device streams the comparables provide.
- Synthetic input is origin- and kind-scoped with every event tagged
  `Physical`/`Synthetic`, and `ScriptedBackend`/`SimulatedController` drive the real
  pipeline deterministically; neither comparable offers the grant policy.

### Disposition

- **Build:** `EV_ABS` touch/tablet translation, then apply the existing pointer and
  touch config, then replace polling with inotify/udev and supervise reader threads.
- **Integrate/reuse:** use libinput for acceleration, tap-to-click, and gestures
  rather than reimplementing them; use libseat/logind for seat handling; bump the
  `evdev` dependency from 0.12 to 0.13; use xkbcommon for non-US layouts.
- **Defer/reject:** non-Linux backends, a gamepad backend until a consumer exists,
  accessibility behaviors (keep the flags), and driver sandboxing until a
  subprocess/IPC architecture is chosen.

### Sources

- https://github.com/libsdl-org/SDL and /releases/tag/3.4.18 (accessed 2026-10-24)
- https://github.com/Smithay/smithay and /tags (accessed 2026-10-24)
- https://github.com/emberian/evdev and its Cargo.toml (accessed 2026-10-24)
- https://gitlab.freedesktop.org/libinput/libinput/-/releases/1.32.0 (accessed 2026-10-24)
- Repo files inspected: README.md, AUDIT_FINDINGS.md, ROADMAP_HONEST.md,
  ARCHITECTURE.md, Cargo.toml, all manifests, `core/src/config.rs`,
  `core/src/source.rs`, `core/src/capture.rs`, `linux/src/linux/*.rs`
