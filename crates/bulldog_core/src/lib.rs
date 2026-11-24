//! Core DSP, pixel buffer, color management, determinism, and plugin traits
//! for **Bulldog Complements**.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod abort;
pub mod backend;
pub mod color;
pub mod determinism;
pub mod dsp;
pub mod error;
pub mod image;
pub mod math;
pub mod param;
pub mod plugin;
pub mod render;

pub use abort::{AbortCheck, AtomicAbort, NeverAbort};
pub use backend::{BackendKind, ComputeBackend};
pub use color::{ColorSpace, PreMultiplication, Rgba32F};
pub use determinism::{DeterministicRng, frame_time_to_key, hash_2d, hash_2d_signed, hash_u64};
pub use error::{CoreError, CoreResult};
pub use image::{BufferPool, HostImageMut, HostImageRef, LinearFrame, PixelFormat};
pub use math::{
    MAX_ALLOCATION_PIXELS, MAX_FRAME_DIMENSION, RectD, RectI, normalized_radius_to_pixels,
    safe_clamp_f32, safe_clamp_f64, safe_div_f32, safe_div_f64, safe_f32_to_i32, safe_f32_to_u8,
    safe_f32_to_u16, sanitize_f32, sanitize_f64,
};
pub use param::{ParamDescriptor, ParamKind, ParamSet, ParamValue};
pub use plugin::{
    EffectContext, EffectDescriptor, EffectPlugin, IdentityClip, RenderContext, RenderInputs,
};
pub use render::{HostRenderInputs, RenderOutcome, render_effect_safe, render_rows_parallel};
