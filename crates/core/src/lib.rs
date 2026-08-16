//! SHER-Input canonical types and orchestration (section 3, 11, 33).
//!
//! `sher_input_core` is backend-independent: it defines the device model, the
//! canonical event model, per-device state tracking, sequencing, coalescing, capture,
//! and configuration, plus the [`InputBackend`] contract a backend implements. It
//! depends on nothing Linux-specific — `sher_input_linux` is one implementation of
//! [`InputBackend`], not a foundation this crate is built on top of.

pub mod backend;
pub mod capture;
pub mod coalesce;
pub mod config;
pub mod device;
pub mod error;
pub mod event;
pub mod gamepad;
pub mod ids;
pub mod keyboard;
pub mod pointer;
pub mod registry;
pub mod service;
pub mod source;
pub mod tablet;
pub mod touch;

pub use backend::{BackendEvent, BackendEventSink, BackendHandle, InputBackend};
pub use capture::{CaptureGuard, CaptureId, CaptureKind, CaptureRegistry, PointerGrabMode};
pub use coalesce::Coalescer;
pub use config::{AccessibilityConfig, InputConfig, KeyboardConfig, PointerConfig, TouchConfig};
pub use device::{
    BackendMetadata, ConnectionState, InputDevice, InputDeviceCapabilities, InputDeviceClass,
};
pub use error::{Error, Result};
pub use event::{InputEvent, InputEventPayload};
pub use gamepad::{GamepadAxis, GamepadButton, GamepadEvent};
pub use ids::{InputDeviceId, SequenceCounter, SequenceNumber, Timestamp};
pub use keyboard::{
    KeyAction, KeyboardEvent, KeyboardLayout, KeyboardState, KeyboardStateTable, LogicalKey,
    Modifiers, PhysicalKey, UsQwertyLayout,
};
pub use pointer::{
    NormalizedPosition, PointerButton, PointerEvent, PointerState, PointerStateTable, ScrollAxis,
};
pub use registry::DeviceRegistry;
pub use service::InputService;
pub use source::{InputSource, SyntheticInputGrant, SyntheticOrigin};
pub use tablet::{TabletEvent, TabletTool};
pub use touch::{ContactId, TouchContact, TouchEvent, TouchPhase, TouchState, TouchStateTable};
