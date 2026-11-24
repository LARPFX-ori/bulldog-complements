//! **Bulldog Sapphirus** — Optical glow, procedural lighting, and cinematic optical FX suite.

#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use bulldog_core::dsp::{MAX_BLUR_RADIUS_PX, gaussian_approx_blur};
use bulldog_core::{
    BufferPool, CoreError, CoreResult, EffectContext, EffectDescriptor, EffectPlugin, IdentityClip,
    LinearFrame, ParamDescriptor, ParamSet, RectD, RenderContext, RenderInputs, Rgba32F,
    normalized_radius_to_pixels, render_rows_parallel, safe_clamp_f32,
};

/// OpenFX identifier for the Sapphirus Optical Glow effect.
pub const OPTICAL_GLOW_ID: &str = "br.com.bulldog.complements.sapphirus.optical_glow";

/// Linear HDR thresholded optical bloom with additive or screen compositing.
#[derive(Debug, Default)]
pub struct OpticalGlowEffect;

impl OpticalGlowEffect {
    fn radius_padding(rect: RectD, params: &ParamSet) -> f64 {
        let intensity = params.get_double("intensity", 0.85, 0.0, 4.0);
        let radius = params.get_double("radius", 0.025, 0.0, 0.25);
        if intensity <= 1e-6 || radius <= 1e-6 {
            return 0.0;
        }
        let ref_dim = rect.width().min(rect.height()).max(1.0);
        (radius * ref_dim * 3.0).min(f64::from(MAX_BLUR_RADIUS_PX) * 3.0)
    }
}

impl EffectPlugin for OpticalGlowEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: OPTICAL_GLOW_ID.to_owned(),
            label: "Bulldog Optical Glow".to_owned(),
            grouping: "Bulldog Sapphirus".to_owned(),
            description: "Linear-light HDR highlight extraction and soft optical bloom composite."
                .to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::double(
                    "threshold",
                    "Threshold",
                    "Linear luminance threshold above which highlights emit glow",
                    0.75,
                    0.0,
                    4.0,
                ),
                ParamDescriptor::double(
                    "intensity",
                    "Intensity",
                    "Optical glow emission gain",
                    0.85,
                    0.0,
                    4.0,
                ),
                ParamDescriptor::double(
                    "radius",
                    "Radius",
                    "Bloom spread normalized to the shorter frame dimension",
                    0.025,
                    0.0,
                    0.25,
                ),
                ParamDescriptor::rgba(
                    "tint",
                    "Tint",
                    "Color multiplier applied to bloom emission",
                    [1.0, 0.96, 0.9, 1.0],
                ),
                ParamDescriptor::choice(
                    "blend_mode",
                    "Blend Mode",
                    "How glow combines with the source frame",
                    0,
                    &["Add", "Screen"],
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let intensity = params.get_double("intensity", 0.85, 0.0, 4.0);
        if intensity <= 1e-6 {
            Some(IdentityClip::Source)
        } else {
            None
        }
    }

    fn region_of_definition(&self, src_rod: RectD, params: &ParamSet, _frame_time: f64) -> RectD {
        let pad = Self::radius_padding(src_rod, params);
        src_rod.expand(pad, pad)
    }

    fn regions_of_interest(
        &self,
        render_window: RectD,
        params: &ParamSet,
        _frame_time: f64,
    ) -> RectD {
        let pad = Self::radius_padding(render_window, params);
        render_window.expand(pad, pad)
    }

    fn render(
        &self,
        inputs: &RenderInputs<'_>,
        output: &mut LinearFrame,
        params: &ParamSet,
        ctx: &RenderContext<'_>,
        pool: &mut BufferPool,
    ) -> CoreResult<()> {
        let Some(src) = inputs.source else {
            return Err(CoreError::MissingInputClip("Source"));
        };
        let threshold = params.get_double("threshold", 0.75, 0.0, 4.0) as f32;
        let intensity = params.get_double("intensity", 0.85, 0.0, 4.0) as f32;
        let radius_norm = params.get_double("radius", 0.025, 0.0, 0.25);
        let [tr, tg, tb, _ta] = params.get_rgba("tint", [1.0, 0.96, 0.9, 1.0]);
        let blend_mode = params.get_choice("blend_mode", 0, 2);

        if intensity <= 1e-6 {
            return output.copy_from(src);
        }

        let width = src.width();
        let height = src.height();
        let mut highlights = pool.acquire(width, height)?;
        let src_pixels = src.pixels();
        let w_usize = width as usize;

        // Soft-knee linear highlight extraction.
        let extract_res = render_rows_parallel(&mut highlights, ctx.abort, |y, row| {
            let start = (y as usize).saturating_mul(w_usize);
            if let Some(s_row) = src_pixels.get(start..start + w_usize) {
                for (s, h_px) in s_row.iter().zip(row.iter_mut()) {
                    let luma = s.luma_rec709();
                    let excess = (luma - threshold).max(0.0);
                    let weight = if luma > 1e-5 { excess / luma } else { 0.0 };
                    *h_px = Rgba32F::new(
                        s.r * weight * s.a,
                        s.g * weight * s.a,
                        s.b * weight * s.a,
                        s.a,
                    );
                }
            }
        });
        if let Err(err) = extract_res {
            pool.release(highlights);
            return Err(err);
        }

        let radius_px = normalized_radius_to_pixels(radius_norm, width, height, MAX_BLUR_RADIUS_PX);
        let mut blurred = pool.acquire(width, height)?;
        let mut scratch = pool.acquire(width, height)?;
        let blur_res = gaussian_approx_blur(
            &highlights,
            &mut blurred,
            &mut scratch,
            radius_px,
            ctx.abort,
        );
        pool.release(highlights);
        pool.release(scratch);
        if let Err(err) = blur_res {
            pool.release(blurred);
            return Err(err);
        }

        output.resize_clear(width, height)?;
        let blur_pixels = blurred.pixels();
        let comp_res = render_rows_parallel(output, ctx.abort, |y, out_row| {
            let start = (y as usize).saturating_mul(w_usize);
            if let (Some(s_row), Some(b_row)) = (
                src_pixels.get(start..start + w_usize),
                blur_pixels.get(start..start + w_usize),
            ) {
                for ((s, b), o) in s_row.iter().zip(b_row.iter()).zip(out_row.iter_mut()) {
                    let gr = (b.r * tr * intensity).max(0.0);
                    let gg = (b.g * tg * intensity).max(0.0);
                    let gb = (b.b * tb * intensity).max(0.0);
                    let (r, g, b_ch) = if blend_mode == 1 {
                        // Screen blend
                        (
                            1.0 - (1.0 - safe_clamp_f32(s.r, 0.0, 1.0))
                                * (1.0 - safe_clamp_f32(gr, 0.0, 1.0)),
                            1.0 - (1.0 - safe_clamp_f32(s.g, 0.0, 1.0))
                                * (1.0 - safe_clamp_f32(gg, 0.0, 1.0)),
                            1.0 - (1.0 - safe_clamp_f32(s.b, 0.0, 1.0))
                                * (1.0 - safe_clamp_f32(gb, 0.0, 1.0)),
                        )
                    } else {
                        // Physical additive HDR blend
                        (s.r + gr, s.g + gg, s.b + gb)
                    };
                    let glow_luma = (0.2126 * gr + 0.7152 * gg + 0.0722 * gb).min(1.0);
                    let out_a = safe_clamp_f32(s.a + glow_luma * (1.0 - s.a), 0.0, 1.0);
                    *o = Rgba32F::new(r, g, b_ch, out_a);
                }
            }
        });
        pool.release(blurred);
        comp_res
    }
}

/// Returns static descriptors for every plugin in the Sapphirus suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Sapphirus suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(OpticalGlowEffect)]
}

bulldog_ofx::register_ofx_suite!((
    SAPPHIRUS_OPTICAL_GLOW,
    c"br.com.bulldog.complements.sapphirus.optical_glow",
    1,
    0,
    || Box::new(OpticalGlowEffect)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, ParamValue, Rgba32F};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn sapphirus_descriptors_and_glow_halo() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = OpticalGlowEffect;
        let mut pool = BufferPool::new();
        let mut src = pool.acquire(24, 24).unwrap();
        for px in src.pixels_mut() {
            *px = Rgba32F::BLACK;
        }
        src.set_pixel(12, 12, Rgba32F::WHITE);

        let mut out = pool.acquire(24, 24).unwrap();
        let mut params = ParamSet::defaults_from(&effect.descriptor().params);
        params.insert("threshold", ParamValue::Double(0.2));
        params.insert("intensity", ParamValue::Double(2.0));
        params.insert("radius", ParamValue::Double(0.15));

        let ctx = RenderContext {
            frame_time: 0.0,
            render_scale: [1.0, 1.0],
            bounds: bulldog_core::RectI::new(0, 0, 24, 24),
            rod: RectD::new(0.0, 0.0, 24.0, 24.0),
            color_space: bulldog_core::ColorSpace::Linear,
            premult: bulldog_core::PreMultiplication::UnPreMultiplied,
            backend: BackendKind::Cpu,
            abort: &NeverAbort,
        };
        let inputs = RenderInputs {
            source: Some(&src),
            source_from: None,
            source_to: None,
        };
        effect
            .render(&inputs, &mut out, &params, &ctx, &mut pool)
            .unwrap();

        // Dark neighbor next to bright pixel receives bloom halo
        assert!(out.get_pixel(13, 12).unwrap().r > 0.0);
        // Identity when intensity == 0
        params.insert("intensity", ParamValue::Double(0.0));
        assert_eq!(effect.is_identity(&params, 0.0), Some(IdentityClip::Source));
    }
}
