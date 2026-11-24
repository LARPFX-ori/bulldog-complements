use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::abort::AbortCheck;
use crate::backend::BackendKind;
use crate::color::{ColorSpace, PreMultiplication, Rgba32F};
use crate::error::{CoreError, CoreResult};
use crate::image::{
    BufferPool, HostImageMut, HostImageRef, LinearFrame, copy_host_bypass, decode_host_to_linear,
    encode_linear_to_host,
};
use crate::math::{RectD, RectI};
use crate::param::ParamSet;
use crate::plugin::{EffectPlugin, IdentityClip, RenderContext, RenderInputs};

/// Host-supplied input image views passed into [`render_effect_safe`].
#[derive(Debug, Clone, Copy, Default)]
pub struct HostRenderInputs<'a> {
    pub source: Option<HostImageRef<'a>>,
    pub source_from: Option<HostImageRef<'a>>,
    pub source_to: Option<HostImageRef<'a>>,
}

/// Describes how a render completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOutcome {
    /// Effect rendered normally.
    Rendered,
    /// Effect reported identity and bypassed rendering via fast host copy.
    IdentityBypass(IdentityClip),
    /// A recoverable error occurred and the primary source was safely passed through.
    ErrorBypass,
}

/// Processes rows of `frame` in parallel using `rayon` while periodically polling `abort`.
///
/// If `abort.is_aborted()` triggers on any worker, remaining rows short-circuit and
/// [`CoreError::Aborted`] is returned without panicking.
pub fn render_rows_parallel<F>(
    frame: &mut LinearFrame,
    abort: &dyn AbortCheck,
    row_fn: F,
) -> CoreResult<()>
where
    F: Fn(u32, &mut [Rgba32F]) + Sync,
{
    let width = frame.width() as usize;
    if width == 0 || frame.height() == 0 {
        return Ok(());
    }
    if abort.is_aborted() {
        return Err(CoreError::Aborted);
    }

    let aborted = AtomicBool::new(false);
    frame
        .pixels_mut()
        .par_chunks_exact_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            if aborted.load(Ordering::Relaxed) {
                return;
            }
            if (y & 15) == 0 && abort.is_aborted() {
                aborted.store(true, Ordering::Relaxed);
                return;
            }
            row_fn(y as u32, row);
        });

    if aborted.load(Ordering::Relaxed) || abort.is_aborted() {
        return Err(CoreError::Aborted);
    }
    Ok(())
}

/// Executes an [`EffectPlugin`] end-to-end against host pixel buffers with:
/// - `IsIdentity` fast bypass,
/// - Host format decode -> straight linear `f32` -> effect -> host format encode,
/// - Reusable scratch buffers via `pool`,
/// - Cooperative `OfxAbort` propagation,
/// - Automatic clean bypass on recoverable non-abort render failures (§0.1).
#[allow(clippy::too_many_arguments)]
pub fn render_effect_safe(
    plugin: &dyn EffectPlugin,
    inputs: &HostRenderInputs<'_>,
    output: &mut HostImageMut<'_>,
    params: &ParamSet,
    frame_time: f64,
    render_scale: [f64; 2],
    color_space: ColorSpace,
    premult: PreMultiplication,
    backend: BackendKind,
    abort: &dyn AbortCheck,
    pool: &mut BufferPool,
) -> CoreResult<RenderOutcome> {
    if abort.is_aborted() {
        return Err(CoreError::Aborted);
    }

    if let Some(identity) = plugin.is_identity(params, frame_time) {
        let clip = match identity {
            IdentityClip::Source => inputs.source,
            IdentityClip::SourceFrom => inputs.source_from,
            IdentityClip::SourceTo => inputs.source_to,
        };
        if let Some(src) = clip {
            copy_host_bypass(&src, output)?;
            return Ok(RenderOutcome::IdentityBypass(identity));
        }
    }

    let width = output.width();
    let height = output.height();

    let mut src_frame = match inputs.source {
        Some(host_src) => {
            let mut buf = pool.acquire(host_src.width(), host_src.height())?;
            decode_host_to_linear(&host_src, &mut buf, color_space, premult, abort)?;
            Some(buf)
        }
        None => None,
    };
    let mut from_frame = match inputs.source_from {
        Some(host_from) => {
            let mut buf = pool.acquire(host_from.width(), host_from.height())?;
            decode_host_to_linear(&host_from, &mut buf, color_space, premult, abort)?;
            Some(buf)
        }
        None => None,
    };
    let mut to_frame = match inputs.source_to {
        Some(host_to) => {
            let mut buf = pool.acquire(host_to.width(), host_to.height())?;
            decode_host_to_linear(&host_to, &mut buf, color_space, premult, abort)?;
            Some(buf)
        }
        None => None,
    };

    let mut out_frame = pool.acquire(width, height)?;
    let bounds = RectI::new(0, 0, width as i32, height as i32);
    let rod = RectD::new(0.0, 0.0, f64::from(width), f64::from(height));

    let ctx = RenderContext {
        frame_time,
        render_scale,
        bounds,
        rod,
        color_space,
        premult,
        backend,
        abort,
    };

    let linear_inputs = RenderInputs {
        source: src_frame.as_ref(),
        source_from: from_frame.as_ref(),
        source_to: to_frame.as_ref(),
    };

    let render_res = plugin.render(&linear_inputs, &mut out_frame, params, &ctx, pool);

    // Return acquired input frames to the pool regardless of render outcome.
    if let Some(f) = src_frame.take() {
        pool.release(f);
    }
    if let Some(f) = from_frame.take() {
        pool.release(f);
    }
    if let Some(f) = to_frame.take() {
        pool.release(f);
    }

    match render_res {
        Ok(()) => {
            let enc_res = encode_linear_to_host(&out_frame, output, color_space, premult, abort);
            pool.release(out_frame);
            enc_res?;
            Ok(RenderOutcome::Rendered)
        }
        Err(CoreError::Aborted) => {
            pool.release(out_frame);
            Err(CoreError::Aborted)
        }
        Err(err) => {
            pool.release(out_frame);
            // Safe bypass fallback: pass primary source through instead of failing violently.
            let fallback_src = inputs.source.or(inputs.source_from).or(inputs.source_to);
            if let Some(src) = fallback_src {
                copy_host_bypass(&src, output)?;
                Ok(RenderOutcome::ErrorBypass)
            } else {
                Err(err)
            }
        }
    }
}
