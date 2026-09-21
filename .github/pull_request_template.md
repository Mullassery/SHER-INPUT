## What this changes and why

<!-- What problem does this solve? Link an issue if one exists. -->

## Testing

<!-- What did you run, and what were the actual results? Don't assert
something works without having run it. If your change touches
crates/linux/src/linux/**, did you test it inside a Linux container (see
README.md's "Building" section)? -->

- [ ] `cargo build --workspace --all-targets`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --check`
- [ ] New behavior has a new test (not just existing tests still passing)

## Architectural boundary check

- [ ] This does not make SHER-Input own windows, surfaces, focus, or
      compositing (that's SHER-Display's job — see `ARCHITECTURE.md`)
- [ ] This does not add a dependency on SHER-Kernel without a concrete,
      stated need
- [ ] If this adds a crate, `Cargo.toml`'s `members`, `README.md`, and
      `ARCHITECTURE.md` are all updated to match

## Docs

- [ ] If this changes what's implemented vs. planned, `README.md` and/or
      `ARCHITECTURE.md`'s "Phased scope" table are updated — stale status
      claims are treated as bugs in this repo
