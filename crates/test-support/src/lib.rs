//! Simulated SHER-Input backend and test fixtures (section 29, 25).
//!
//! Everything here is a real, if fake, implementation of the same contracts a
//! hardware backend uses — `sher_input_linux` and `sher_input_test` both implement
//! [`sher_input_core::InputBackend`] and both talk to `InputService` only through
//! [`sher_input_core::BackendEventSink`]. Nothing in SHER-Input's own tests, or in a
//! downstream consumer's tests, needs Linux or real devices attached.

pub mod backend;
pub mod controller;
pub mod devices;

pub use backend::ScriptedBackend;
pub use controller::SimulatedController;
pub use devices::{keyboard_device, mouse_device, touchscreen_device};
