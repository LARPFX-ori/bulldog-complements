/// Execution backend selected for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendKind {
    /// Portable multi-threaded CPU reference backend (always available).
    #[default]
    Cpu,
    /// Portable GPU backend (`wgpu`/WGSL) with automatic fallback to CPU.
    GpuWithCpuFallback,
}

/// Runtime backend descriptor enforcing mandatory CPU fallback (AGENTS.md §3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputeBackend {
    requested: BackendKind,
    active: BackendKind,
}

impl ComputeBackend {
    /// Initializes a CPU-only compute backend.
    #[must_use]
    pub const fn cpu() -> Self {
        Self {
            requested: BackendKind::Cpu,
            active: BackendKind::Cpu,
        }
    }

    /// Requests a backend, falling back safely to CPU whenever GPU acceleration
    /// is unavailable or uninitialized.
    #[must_use]
    pub const fn resolve(requested: BackendKind, gpu_available: bool) -> Self {
        let active = match (requested, gpu_available) {
            (BackendKind::GpuWithCpuFallback, true) => BackendKind::GpuWithCpuFallback,
            _ => BackendKind::Cpu,
        };
        Self { requested, active }
    }

    /// Returns the requested backend mode.
    #[must_use]
    pub const fn requested(self) -> BackendKind {
        self.requested
    }

    /// Returns the active backend actually used during rendering.
    #[must_use]
    pub const fn active(self) -> BackendKind {
        self.active
    }
}

impl Default for ComputeBackend {
    fn default() -> Self {
        Self::cpu()
    }
}
