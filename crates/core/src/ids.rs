use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Identifies an [`InputDevice`](crate::device::InputDevice) for the lifetime of its
/// connection. Backends mint one per physical (or virtual) device at discovery time;
/// it carries no meaning outside SHER-Input (Linux event-node paths, HID ids, etc. are
/// backend metadata, not part of this id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InputDeviceId(Uuid);

impl InputDeviceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for InputDeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InputDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Monotonic per-device-independent counter used to order events that land on the
/// same timestamp tick (section 12: ordering must not depend on timestamp resolution
/// alone). Assigned once, by [`InputService`](crate::service::InputService), never by
/// backends — this is what makes "Event A happened before Event B" answerable even
/// when two devices produce events in the same clock tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

#[derive(Debug, Default)]
pub struct SequenceCounter(AtomicU64);

impl SequenceCounter {
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn next(&self) -> SequenceNumber {
        SequenceNumber(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

/// Nanoseconds since [`UNIX_EPOCH`], captured as close to hardware event arrival as
/// the backend allows. Used for cross-device wall-clock comparison and latency
/// measurement (section 13, section 28) — [`SequenceNumber`] is the tie-breaker and
/// the authority for same-timestamp ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn now() -> Self {
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self(dur.as_nanos() as u64)
    }

    pub fn as_nanos(&self) -> u64 {
        self.0
    }
}
