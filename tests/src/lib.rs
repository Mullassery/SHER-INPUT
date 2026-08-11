//! Cross-crate integration tests for SHER-Input (section 29, 33). Deliberately not a
//! real library — the tests in `tests/contract.rs` are the point of this package, kept
//! separate from `crates/core` so they exercise the public API the same way an
//! external consumer (SHER-Display, Aurora, or a downstream tool) would.
