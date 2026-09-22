# ROADMAP_HONEST

This repository has no separate `ROADMAP.md` — `ARCHITECTURE.md`'s "Phased
scope" table already tracks phase status. This file is the honesty
supplement: what's actually verified, what's untested, and a concrete
technical-debt inventory with file:line references. Nothing here uses hedge
language for things that are simply not built — if it's not built, it says
so.

## Phase status (mirrors ARCHITECTURE.md, restated with verification detail)

| Phase | Scope | Status | Verified how |
|---|---|---|---|
| 1 | Linux backend, device registry, keyboard, pointer, scrolling, normalization, sequencing, hotplug (poll-based), diagnostics | **Done** | 36 platform-independent tests (`cargo test --workspace`, run by this pass on macOS, all passing) + 12 Linux-only tests, previously run inside a real `rust:latest` Linux container per README — **not re-run inside Linux by this pass**; only re-verified by reading the CI config and trusting the ubuntu-latest CI leg, which this pass did not trigger. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` both pass clean on macOS as of this pass. |
| 2 | SHER-Display integration: focus, pointer targeting, capture, multi-monitor coordinates | **Not started** as real cross-crate wiring. The *contract* is proven in this repo's own `tests/tests/contract.rs` against a hand-written mock router, not SHER-Display's real code. SHER-Display's own repo separately verifies its `sher_display_input` crate builds against this repo's API (56/56 tests passing there, per README) — that verification was not re-run by this pass; it is a claim carried over from README.md that this pass did not independently re-execute. |
| 3 | Aurora integration: shortcuts, menus, window manipulation via SHER-Display | **Not started.** No code, no design doc beyond a one-line mention in the phase table. |
| 4 | Touch, multi-touch, tablet, gamepad backends; accessibility behaviors | **Not built.** Types exist in `crates/core/src/touch.rs`, `tablet.rs`, `gamepad.rs`; no backend produces these events. Accessibility config flags exist (`crates/core/src/config.rs`) but no behavior reads them. |
| 5 | Synthetic input, remote input, AI-agent-controlled input, advanced security policy | **Partially built.** The grant/origin/kind-scoping model exists and is enforced (`crates/core/src/source.rs`, tested in `tests/tests/service.rs::synthetic_input_is_tagged_and_rejected_without_a_grant`). Policy for *who gets to issue a grant* does not exist — SHER-Input only enforces scope once a grant already exists. |

## What was actually validated in this pass (2026-09-20)

- `cargo test --workspace` on macOS: 36 tests pass (22 in `sher_input_core`,
  3 in `backend_crash_recovery.rs`, 6 in `tests/service.rs`, 5 in
  `tests/tests/contract.rs`). This matches the README's claim of "36
  platform-independent" tests. The 12 Linux-only tests in
  `crates/linux/src/linux/{reader,keymap}.rs` were **not run** — this host
  is macOS, and no Docker/Linux container was used in this pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean, no
  warnings.
- `cargo fmt --check`: clean.
- `cargo audit`: **could not run** — this sandbox has no network access to
  fetch the RustSec advisory database (`git fetch` to
  `github.com/RustSec/advisory-db` failed with a network error). Dependency
  vulnerability status is genuinely unknown as of this pass. A CI job has
  been added to run this automatically going forward (see
  `.github/workflows/ci.yml`), but it has not yet produced a real result.
- `actionlint` against `.github/workflows/ci.yml`: passes, no findings.
- Secret scan (grep for API keys/passwords/private-key headers across
  tracked source/config files): nothing found.

## Technical debt (concrete, file:line)

1. **Reader-thread death is invisible unless the device is unplugged.**
   `crates/linux/src/linux/backend.rs:46-58` (`supervise`) polls for new
   devices every 500ms and calls `report_removed` (line 87), which only
   checks `path.exists()` (line 90) — it has no way to detect that a
   per-device reader thread (spawned at line 73) has panicked or exited
   while the device node is still present. A reader thread that dies for
   any reason other than physical unplug leaves that device silently dead
   with no `DeviceRemoved` event and no restart, until the device is
   physically replugged. This is distinct from (and narrower than) the
   already-documented `InputService`-level crash recovery
   (`BackendHandle::is_finished()`/`stop_and_check_panicked()` in
   `crates/core/src/backend.rs`) — that machinery detects a whole backend
   thread dying, not one reader thread among several inside a running
   backend. Worth a dedicated follow-up: either join/poll reader threads in
   `supervise`, or accept and document this as a known Phase 1 limitation
   more prominently than the current one-line ARCHITECTURE.md mention.
2. **No sandboxing for code parsing untrusted device data.**
   `crates/linux/src/linux/backend.rs:27-34` (`LinuxBackend::spawn`) and
   `reader.rs`'s per-device threads run in-process with no seccomp,
   namespace, or subprocess isolation between the code parsing raw evdev
   input and the rest of the process. Documented in `SECURITY.md` and
   `ARCHITECTURE.md`; fixing it is real, non-trivial work (subprocess
   architecture + IPC), not a follow-up patch.
3. **No fuzzing at the evdev boundary.** No `cargo-fuzz`/`proptest` target
   exists anywhere in this repo (`grep -r cargo-fuzz` and `grep -r proptest`
   both return nothing). `keymap::map_key`/`map_button` in
   `crates/linux/src/linux/keymap.rs` and `translate_batch` in `reader.rs`
   are the natural fuzz targets — they take `evdev::InputEvent` values and
   have already had three real bugs found by manual Linux-container
   testing (see CHANGELOG's 0.2.0 entry), which is exactly the class of bug
   fuzzing would catch faster.
4. **Fail-fast lock poisoning throughout `sher_input_core`.** Every access
   to shared state uses `.expect("... lock poisoned")` —
   `crates/core/src/service.rs` (13 call sites, e.g. lines 106, 110, 114,
   186, 218-230), `crates/core/src/registry.rs` (lines 22, 33, 41, 49, 58,
   66), `crates/core/src/capture.rs` (lines 61, 88, 95, 103, 113, 119). This
   is a defensible design choice (a poisoned lock means an invariant was
   already broken by a panicking thread), but it means a single unexpected
   panic anywhere while holding one of these locks escalates into every
   subsequent caller panicking too, rather than the failure being contained
   to the one backend/device that caused it. Not a bug, but worth a
   deliberate decision (documented here rather than silently relied upon)
   in any future work that increases how many threads touch `InputService`
   concurrently.
5. **Hotplug topology discovery is polling, not event-driven.**
   `crates/linux/src/linux/backend.rs:9` (`POLL_INTERVAL =
   Duration::from_millis(500)`) means a hotplugged device can take up to
   500ms to be discovered. Already documented in `ARCHITECTURE.md` as a
   deliberate Phase 1→2 deferral; restated here because it's a real latency
   bound, not a hypothetical one.
6. **Dependency vulnerability status unknown.** No `cargo audit` has ever
   been run against this repo (verified: no CI step existed before this
   pass, and this pass's own attempt failed on network access — see
   above). `.github/dependabot.yml` did not exist before this pass either,
   so there was no automated dependency-update signal at all.
7. **Version drift between `Cargo.toml` and git tags.** `Cargo.toml` has
   been at `0.3.0` since commit `4465206`'s successors, but the only git tag
   is `v0.2.0` (pointing at `4465206` itself, which actually shipped as
   `0.2.0`). Everything since — crash-recovery, relicensing, cross-repo
   compat docs, use-cases section — is unreleased, untagged `0.3.0` work.
   Not a bug, but a real release-hygiene gap: there is currently no tag a
   consumer could pin to that includes the crash-recovery work.
8. **Cross-repo integration claims are not independently re-verified by
   this pass.** README.md and this file both state SHER-Display's
   `sher_display_input` builds and passes 56/56 tests against this repo's
   current API. That claim originates from prior work in this repo's own
   history (commit `c982454`), not from anything this pass re-ran — this
   pass did not check out or build `SHER-Display`. Treat that specific
   claim as "documented, last independently verified at commit `c982454`,"
   not as continuously verified by CI (there is no CI job in this repo or
   `SHER-Display`'s that builds both repos together).

## Deliberately not fixed in this pass

Per the scope of this pass (documentation/disclosure-first), none of the
above eight items were fixed — they need dedicated follow-up work, not a
drive-by patch, and are recorded here specifically so a future session can
pick one and scope it properly.

## Fixed in a later quick-fix pass (2026-09-22)

9. ~~**`submit_synthetic` let any grant add/remove devices, regardless of
   scope.**~~ **Fixed.** `crates/core/src/service.rs`'s `submit_synthetic`
   (previously line 166) matched `BackendEvent::DeviceAdded(_) |
   BackendEvent::DeviceRemoved(_) => true` unconditionally — a grant scoped
   to *only* keyboard input (e.g. `SyntheticInputGrant::new(origin)
   .with_keyboard()`) could still call `submit_synthetic` with a
   `DeviceAdded`/`DeviceRemoved` event and have it silently accepted and
   applied to the registry, because `SyntheticInputGrant` has no field
   scoping device management at all. This directly contradicted the
   documented invariant in `crates/core/src/source.rs`: "There is
   deliberately no `SyntheticInputGrant::unrestricted()`... every grant is
   scoped to one origin and one event kind." Fixed by folding
   `DeviceAdded`/`DeviceRemoved` into the same `=> false` arm as
   `Tablet`/`Gamepad` (kinds no grant can cover), so synthetic hotplug is
   denied outright rather than implicitly always-allowed. Physical hotplug
   (the real path devices are added/removed through, via `InputService::
   ingest`) is untouched — this only closes the synthetic-input side
   channel. Proven by a new test,
   `synthetic_device_added_and_removed_are_rejected_because_no_grant_scope_covers_device_management`
   in `crates/core/tests/service.rs`, which fails against the pre-fix code
   (`submit_synthetic` returned `Ok(())` and registered the spoofed device)
   and passes after.
