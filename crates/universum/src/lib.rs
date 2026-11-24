//! **Bulldog Universum** — Retro analog signal, glitch, CRT scanlines, and stylized motion FX.

#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use core::f32::consts::TAU;

use bulldog_core::{
    BufferPool, CoreError, CoreResult, EffectContext, EffectDescriptor, EffectPlugin, IdentityClip,
    LinearFrame, ParamDescriptor, ParamSet, RenderContext, RenderInputs, Rgba32F,
    frame_time_to_key, hash_2d_signed, render_rows_parallel, safe_clamp_f32, safe_div_f32,
};

/// OpenFX identifier for the Universum Analog Signal Drift effect.
pub const SIGNAL_DRIFT_ID: &str = "br.com.bulldog.complements.universum.signal_drift";

/// Deterministic analog CRT/tape signal effect: horizontal chromatic separation,
/// resolution-independent scanlines, line jitter, and frame-locked static.
#[derive(Debug, Default)]
pub struct SignalDriftEffect;

impl EffectPlugin for SignalDriftEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: SIGNAL_DRIFT_ID.to_owned(),
            label: "Bulldog Signal Drift".to_owned(),
            grouping: "Bulldog Universum".to_owned(),
            description:
                "Deterministic analog chromatic drift, scanline modulation, and line noise."
                    .to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::double(
                    "intensity",
                    "Intensity",
                    "Master wet/dry analog signal intensity",
                    0.45,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::double(
                    "chroma_shift",
                    "Chroma Shift",
                    "Normalized horizontal RGB channel offset",
                    0.008,
                    0.0,
                    0.05,
                ),
                ParamDescriptor::double(
                    "scanline_depth",
                    "Scanline Depth",
                    "Contrast of horizontal CRT scanlines",
                    0.25,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::double(
                    "noise_amount",
                    "Signal Noise",
                    "Deterministic luminance noise amplitude",
                    0.08,
                    0.0,
                    0.5,
                ),
                ParamDescriptor::int(
                    "seed",
                    "Seed",
                    "Deterministic seed for line jitter and noise",
                    13,
                    0,
                    999_999,
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let intensity = params.get_double("intensity", 0.45, 0.0, 1.0);
        if intensity <= 1e-6 {
            Some(IdentityClip::Source)
        } else {
            None
        }
    }

    fn render(
        &self,
        inputs: &RenderInputs<'_>,
        output: &mut LinearFrame,
        params: &ParamSet,
        ctx: &RenderContext<'_>,
        _pool: &mut BufferPool,
    ) -> CoreResult<()> {
        let Some(src) = inputs.source else {
            return Err(CoreError::MissingInputClip("Source"));
        };
        let intensity = params.get_double("intensity", 0.45, 0.0, 1.0) as f32;
        let chroma = params.get_double("chroma_shift", 0.008, 0.0, 0.05) as f32 * intensity;
        let scanline_depth = params.get_double("scanline_depth", 0.25, 0.0, 1.0) as f32 * intensity;
        let noise_amount = params.get_double("noise_amount", 0.08, 0.0, 0.5) as f32 * intensity;
        let seed = params.get_int("seed", 13, 0, 999_999) as u64;
        let frame_key = frame_time_to_key(ctx.frame_time);

        let width = output.width();
        let height = output.height();
        let inv_w = safe_div_f32(1.0, width as f32, 1.0);
        let inv_h = safe_div_f32(1.0, height as f32, 1.0);

        // Fixed 240 virtual scanlines across frame height for resolution independence (§3.2).
        const VIRTUAL_SCANLINES: f32 = 240.0;

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let v = (y as f32 + 0.5) * inv_h;
            let band_id = (v * VIRTUAL_SCANLINES).floor() as i32;
            let line_jitter = hash_2d_signed(band_id, 17, frame_key, seed) * chroma * 0.45;
            let scan_wave = 0.5 + 0.5 * (v * VIRTUAL_SCANLINES * TAU).sin();
            let scan_gain = 1.0 - scanline_depth * (1.0 - scan_wave);

            for (x, out_px) in out_row.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) * inv_w;
                let base_u = u + line_jitter;

                let r_px = src.sample_bilinear(base_u + chroma, v, true);
                let g_px = src.sample_bilinear(base_u, v, true);
                let b_px = src.sample_bilinear(base_u - chroma, v, true);

                let noise = if noise_amount > 1e-6 {
                    hash_2d_signed(x as i32, y as i32, frame_key, seed ^ 0x55AA) * noise_amount
                } else {
                    0.0
                };

                let r = (r_px.r * scan_gain + noise).max(0.0);
                let g = (g_px.g * scan_gain + noise).max(0.0);
                let b = (b_px.b * scan_gain + noise).max(0.0);
                let a = safe_clamp_f32(g_px.a, 0.0, 1.0);
                *out_px = Rgba32F::new(r, g, b, a);
            }
        })
    }
}

/// Returns static descriptors for every plugin in the Universum suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Universum suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(SignalDriftEffect)]
}

bulldog_ofx::register_ofx_suite!((
    UNIVERSUM_SIGNAL_DRIFT,
    c"br.com.bulldog.complements.universum.signal_drift",
    1,
    0,
    || Box::new(SignalDriftEffect)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, ParamValue, RectD, RectI};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn signal_drift_valid_and_random_access_deterministic() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = SignalDriftEffect;
        let mut params = ParamSet::defaults_from(&effect.descriptor().params);
        params.insert("intensity", ParamValue::Double(0.0));
        assert_eq!(effect.is_identity(&params, 5.0), Some(IdentityClip::Source));

        params.insert("intensity", ParamValue::Double(0.8));
        let mut pool = BufferPool::new();
        let mut src = pool.acquire(24, 24).unwrap();
        for px in src.pixels_mut() {
            *px = Rgba32F::new(0.4, 0.4, 0.4, 1.0);
        }
        let ctx = RenderContext {
            frame_time: 24.0,
            render_scale: [1.0, 1.0],
            bounds: RectI::new(0, 0, 24, 24),
            rod: RectD::new(0.0, 0.0, 24.0, 24.0),
            color_space: bulldog_core::ColorSpace::Linear,
            premult: bulldog_core::PreMultiplication::Opaque,
            backend: BackendKind::Cpu,
            abort: &NeverAbort,
        };
        let inputs = RenderInputs {
            source: Some(&src),
            source_from: None,
            source_to: None,
        };
        let mut a = pool.acquire(24, 24).unwrap();
        let mut b = pool.acquire(24, 24).unwrap();
        effect
            .render(&inputs, &mut a, &params, &ctx, &mut pool)
            .unwrap();
        effect
            .render(&inputs, &mut b, &params, &ctx, &mut pool)
            .unwrap();
        assert_eq!(a, b);
    }
}
