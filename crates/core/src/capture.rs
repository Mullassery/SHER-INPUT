use crate::error::{Error, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// What class of input a capture applies to (section 16-17). SHER-Input tracks
/// captures per-kind, not per-device — SHER-Display doesn't need to know which
/// physical keyboard or mouse produced the events it has captured, only that
/// keyboard/pointer/touch input is currently exclusive to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureKind {
    Pointer,
    Keyboard,
    Touch,
}

/// Distinguishes a plain grab (still delivered as normalized position/motion) from a
/// locked pointer (relative-motion-only, position frozen — the mode fullscreen games
/// and pointer-lock web content need, section 6/16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerGrabMode {
    Confined,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CaptureId(u64);

#[derive(Debug, Clone)]
struct ActiveCapture {
    id: CaptureId,
    owner: String,
    reason: String,
    pointer_mode: Option<PointerGrabMode>,
}

/// SHER-Display's interface for requesting exclusive input (section 16). SHER-Input
/// does not decide *who* should be granted a capture — that policy (is this a
/// legitimate drag/resize/modal/fullscreen transition) lives entirely in SHER-Display
/// — it only enforces that a capture is explicit, single-owner, and revocable, and
/// exposes the current state for diagnostics (section 28).
#[derive(Debug, Default)]
pub struct CaptureRegistry {
    next_id: AtomicU64,
    active: RwLock<HashMap<CaptureKind, ActiveCapture>>,
}

impl CaptureRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn acquire(
        self: &Arc<Self>,
        kind: CaptureKind,
        owner: impl Into<String>,
        reason: impl Into<String>,
        pointer_mode: Option<PointerGrabMode>,
    ) -> Result<CaptureGuard> {
        let mut active = self.active.write().expect("capture lock poisoned");
        if let Some(existing) = active.get(&kind) {
            return Err(Error::GrabDenied(format!(
                "{kind:?} already captured by '{}'",
                existing.owner
            )));
        }
        let id = CaptureId(self.next_id.fetch_add(1, Ordering::Relaxed));
        active.insert(
            kind,
            ActiveCapture {
                id,
                owner: owner.into(),
                reason: reason.into(),
                pointer_mode,
            },
        );
        Ok(CaptureGuard {
            registry: Arc::clone(self),
            kind,
            id,
        })
    }

    pub fn is_captured(&self, kind: CaptureKind) -> bool {
        self.active.read().expect("capture lock poisoned").contains_key(&kind)
    }

    pub fn pointer_grab_mode(&self) -> Option<PointerGrabMode> {
        self.active
            .read()
            .expect("capture lock poisoned")
            .get(&CaptureKind::Pointer)
            .and_then(|c| c.pointer_mode)
    }

    pub fn owner_of(&self, kind: CaptureKind) -> Option<String> {
        self.active
            .read()
            .expect("capture lock poisoned")
            .get(&kind)
            .map(|c| c.owner.clone())
    }

    /// Surfaced by `sher-input-monitor` (section 28) so a stuck or unexpected grab is
    /// diagnosable without instrumenting SHER-Display itself.
    pub fn reason_of(&self, kind: CaptureKind) -> Option<String> {
        self.active
            .read()
            .expect("capture lock poisoned")
            .get(&kind)
            .map(|c| c.reason.clone())
    }

    fn release(&self, kind: CaptureKind, id: CaptureId) {
        let mut active = self.active.write().expect("capture lock poisoned");
        if let Some(existing) = active.get(&kind) {
            if existing.id == id {
                active.remove(&kind);
            }
        }
    }
}

/// RAII handle to an active capture. Dropping it — or calling
/// [`CaptureGuard::release`] explicitly — is the only way a capture ends, which is
/// what makes captures revocable by construction rather than by convention (section
/// 16).
#[must_use = "dropping this immediately releases the capture"]
pub struct CaptureGuard {
    registry: Arc<CaptureRegistry>,
    kind: CaptureKind,
    id: CaptureId,
}

impl CaptureGuard {
    pub fn kind(&self) -> CaptureKind {
        self.kind
    }

    pub fn release(self) {
        // Drop performs the release; this just gives callers an explicit name for it.
    }
}

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        self.registry.release(self.kind, self.id);
    }
}
