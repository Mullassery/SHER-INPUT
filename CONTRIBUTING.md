# Contributing to SHER-Input

SHER-Input is Phase 1 of a five-phase plan (see [ARCHITECTURE.md](ARCHITECTURE.md)'s
"Phased scope" table and [README.md](README.md)'s "What's actually
implemented"). Read both before contributing — they describe what's actually
built versus deferred, and the architectural boundaries that must not be
crossed.

## Before you start

- Read [ARCHITECTURE.md](ARCHITECTURE.md)'s "Layering" and "Architectural
  boundary" sections first. SHER-Input owns device discovery, normalization,
  state tracking, sequencing, and the capture/synthetic-input contract. It
  never owns windows, surfaces, focus, or compositing — that's
  SHER-Display's job. It never talks to SHER-Kernel today, and any future
  dependency on SHER-Kernel (e.g. device handles for a native backend) is
  added only when a concrete need exists, not speculatively. If you're
  about to make SHER-Input reach past `/dev/input` into a display/window
  concept, or make it depend on SHER-Kernel without a concrete need — stop
  and open an issue first.
- Read the "Phased scope" table in `ARCHITECTURE.md`. Touch, tablet, and
  gamepad have types but no backend (deliberate Phase 4 deferral); Aurora
  integration (Phase 3) hasn't started. Don't build ahead of the current
  phase without discussing it in an issue first.
- This repo builds standalone — it does not need sibling SHER repos checked
  out. (SHER-Display depends on this repo via a relative path, not the other
  way around.)

## Development setup

```bash
git clone https://github.com/Mullassery/SHER-INPUT SHER-Input
cd SHER-Input
cargo build --workspace
cargo test --workspace
```

The Linux/evdev backend (`sher_input_linux`) only compiles its real logic
under `cfg(target_os = "linux")`; on any other host it's a stub returning
`Error::Unsupported`. To build and test the real Linux backend from a
non-Linux host, use Docker — see README.md's "Building" section for the
exact command CI runs on `ubuntu-latest`.

## Before opening a PR

Run the same checks CI runs, from the repo root:

```bash
cargo build --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

All four must pass with zero warnings. If your change touches
`crates/linux/src/linux/**`, also run it inside a Linux container (see
README.md) — code that only compiles on macOS's stub path is not verified.

## What to include in a PR

- New code needs tests. Every crate in this workspace currently has direct
  unit test coverage; a PR that adds behavior without a test for it will be
  asked to add one.
- If your change adds a new crate, update `Cargo.toml`'s `members` list and
  the crate list in `README.md` and `ARCHITECTURE.md`.
- Do not claim something works in a doc comment or README if it isn't
  covered by a test. This repo's README documents test counts and exactly
  what's verified on real Linux versus only claimed — if you're not sure
  whether something is tested, say so in the PR description rather than
  asserting it works.

## Reporting bugs / requesting features

Use the issue templates under `.github/ISSUE_TEMPLATE/`. There is no
mailing list or chat for this project yet — GitHub issues are the only
channel.

## Security issues

Do not open a public issue for a security concern — see
[SECURITY.md](SECURITY.md).

## License

By contributing, you agree your contributions are licensed under the
[Apache License 2.0](LICENSE), matching the rest of the repo.
