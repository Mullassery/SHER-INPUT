//! SHER-Input's Linux/evdev backend (section 25).
//!
//! This crate is the *only* place SHER-Input allows Linux-specific types and
//! assumptions to exist. It implements `sher_input_core::InputBackend`; nothing else
//! in the workspace depends on it, and it depends on nothing above `sher_input_core`.
//! On any non-Linux target (this workspace is developed on macOS) `LinuxBackend`
//! compiles to a stub that reports `Error::Unsupported` — use
//! `sher_input_test::ScriptedBackend` for development there instead.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxBackend;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::LinuxBackend;
