use crate::abort::AbortCheck;
use crate::backend::BackendKind;
use crate::color::{ColorSpace, PreMultiplication};
use crate::error::CoreResult;
use crate::image::{BufferPool, LinearFrame};
use crate::math::{RectD, RectI};
use crate::param::{ParamDescriptor, ParamSet};

/// Supported OpenFX image effect contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContext {
    /// Standard single-input filter (`OfxImageEffectContextFilter`).
    Filter,
    /// Two-input timeline transition (`OfxImageEffectContextTransition`) with mandatory
    /// `Transition` double parameter in `0.0..=1.0`.
    Transition,
    /// Zero-input procedural generator (`OfxImageEffectContextGenerator`).
    Generator,
    /// General multi-input effect (`OfxImageEffectContextGeneral`).
    General,
}

/// Which input clip an effect is identical to when [`EffectPlugin::is_identity`] triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityClip {
    /// Primary `Source` clip of a filter.
    Source,
    /// Outgoing `SourceFrom` clip of a transition (typically at `Transition == 0.0`).
    SourceFrom,
    /// Incoming `SourceTo` clip of a transition (typically at `Transition == 1.0`).
    SourceTo,
}

/// Static metadata describing an OpenFX plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectDescriptor {
    /// Globally unique OpenFX identifier (`br.com.bulldog.complements.<suite>.<effect>`).
    pub identifier: String,
    /// Human-readable plugin label shown in the NLE.
    pub label: String,
    /// Category / menu grouping shown in the NLE (e.g. `Bulldog Continuus`).
    pub grouping: String,
    /// Short functional description.
    pub description: String,
    /// Semantic major API version advertised to OpenFX.
    pub version_major: u32,
    /// Semantic minor API version advertised to OpenFX.
    pub version_minor: u32,
    /// OpenFX context supported by this plugin.
    pub context: EffectContext,
    /// Ordered parameter definitions.
    pub params: Vec<ParamDescriptor>,
}

/// Linear-light input frames available during an effect render pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderInputs<'a> {
    /// Primary clip for `Filter` / `General` contexts.
    pub source: Option<&'a LinearFrame>,
    /// Outgoing clip for `Transition` context (`SourceFrom`).
    pub source_from: Option<&'a LinearFrame>,
    /// Incoming clip for `Transition` context (`SourceTo`).
    pub source_to: Option<&'a LinearFrame>,
}

/// Immutable per-frame context provided to [`EffectPlugin::render`].
#[derive(Clone, Copy)]
pub struct RenderContext<'a> {
    /// Timeline frame/time coordinate.
    pub frame_time: f64,
    /// Host proxy render scale `(sx, sy)` where `(1.0, 1.0)` is full resolution.
    pub render_scale: [f64; 2],
    /// Output pixel bounds in canonical pixel space.
    pub bounds: RectI,
    /// Source region of definition in canonical coordinates.
    pub rod: RectD,
    /// Color transfer space of the host buffers.
    pub color_space: ColorSpace,
    /// Host premultiplication convention.
    pub premult: PreMultiplication,
    /// Active compute backend.
    pub backend: BackendKind,
    /// Cooperative cancellation token (`OfxAbort`).
    pub abort: &'a dyn AbortCheck,
}

impl core::fmt::Debug for RenderContext<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RenderContext")
            .field("frame_time", &self.frame_time)
            .field("render_scale", &self.render_scale)
            .field("bounds", &self.bounds)
            .field("rod", &self.rod)
            .field("color_space", &self.color_space)
            .field("premult", &self.premult)
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

/// Pure-Rust, thread-safe, panic-free plugin contract implemented by every Bulldog effect.
pub trait EffectPlugin: Send + Sync {
    /// Returns the plugin's static descriptor and parameter specification.
    fn descriptor(&self) -> EffectDescriptor;

    /// Reports whether the effect is a no-op at `(params, frame_time)` and which input
    /// clip can be passed through untouched by the host (`OfxImageEffectActionIsIdentity`).
    fn is_identity(&self, _params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        None
    }

    /// Computes the output Region of Definition (RoD) from the primary input RoD.
    ///
    /// Effects that expand image bounds (blur, glow, drop shadow) override this method.
    fn region_of_definition(&self, src_rod: RectD, _params: &ParamSet, _frame_time: f64) -> RectD {
        src_rod
    }

    /// Computes the required source Region of Interest (RoI) for a requested output window.
    ///
    /// Effects with spatial kernels override this method so tiles avoid seam artifacts.
    fn regions_of_interest(
        &self,
        render_window: RectD,
        _params: &ParamSet,
        _frame_time: f64,
    ) -> RectD {
        render_window
    }

    /// Renders the effect in straight linear `f32` RGBA into `output`.
    fn render(
        &self,
        inputs: &RenderInputs<'_>,
        output: &mut LinearFrame,
        params: &ParamSet,
        ctx: &RenderContext<'_>,
        pool: &mut BufferPool,
    ) -> CoreResult<()>;
}
