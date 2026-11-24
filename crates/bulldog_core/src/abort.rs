use std::sync::atomic::{AtomicBool, Ordering};

/// Cooperative cancellation hook bridging OpenFX `abort()` into CPU/GPU passes.
pub trait AbortCheck: Sync {
    /// Returns `true` if the host requested immediate cancellation of this render.
    fn is_aborted(&self) -> bool;
}

/// No-op cancellation check used when no abort hook is attached.
#[derive(Debug, Default, Clone, Copy)]
pub struct NeverAbort;

impl AbortCheck for NeverAbort {
    #[inline]
    fn is_aborted(&self) -> bool {
        false
    }
}

/// Thread-safe cancellation flag useful in tests and asynchronous render hosts.
#[derive(Debug, Default)]
pub struct AtomicAbort {
    flag: AtomicBool,
}

impl AtomicAbort {
    /// Creates a new cancellation flag initialized to `aborted`.
    #[must_use]
    pub const fn new(aborted: bool) -> Self {
        Self {
            flag: AtomicBool::new(aborted),
        }
    }

    /// Signals cancellation to any running render pass.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
}

impl AbortCheck for AtomicAbort {
    #[inline]
    fn is_aborted(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}
