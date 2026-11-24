//! **Bulldog Dulcia Nitentia** — Color grading, film emulation, optical vignette, and grain suite.

#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use bulldog_core::{
    BufferPool, CoreError, CoreResult, EffectContext, EffectDescriptor, EffectPlugin, IdentityClip,
    LinearFrame, ParamDescriptor, ParamSet, RenderContext, RenderInputs, Rgba32F,
    frame_time_to_key, hash_2d_signed, render_rows_parallel, safe_clamp_f32, safe_div_f32,
};

/// OpenFX identifier for the Dulcia Nitentia Optical Vignette & Grain effect.
pub const OPTICAL_VIGNETTE_ID: &str = "br.com.bulldog.complements.dulcia_nitentia.optical_vignette";

/// Resolution-independent optical lens vignette combined with deterministic film grain.
#[derive(Debug, Default)]
pub struct OpticalVignetteEffect;

impl EffectPlugin for OpticalVignetteEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: OPTICAL_VIGNETTE_ID.to_owned(),
            label: "Bulldog Optical Vignette".to_owned(),
            grouping: "Bulldog Dulcia Nitentia".to_owned(),
            description: "Natural optical cosine-fourth vignette and deterministic temporal film grain in linear light.".to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::double(
                    "strength",
                    "Vignette Strength",
                    "Edge light falloff amount",
                    0.35,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::double(
                    "softness",
                    "Softness",
                    "Transition softness from center to corners",
                    0.5,
                    0.05,
                    1.0,
                ),
                ParamDescriptor::double(
                    "grain",
                    "Film Grain",
                    "Deterministic luminance-weighted film grain intensity",
                    0.02,
                    0.0,
                    0.25,
                ),
                ParamDescriptor::int(
                    "seed",
                    "Seed",
                    "Deterministic random seed for grain pattern",
                    42,
                    0,
                    999_999,
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let strength = params.get_double("strength", 0.35, 0.0, 1.0);
        let grain = params.get_double("grain", 0.02, 0.0, 0.25);
        if strength <= 1e-6 && grain <= 1e-6 {
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
        let strength = params.get_double("strength", 0.35, 0.0, 1.0) as f32;
        let softness = params.get_double("softness", 0.5, 0.05, 1.0) as f32;
        let grain = params.get_double("grain", 0.02, 0.0, 0.25) as f32;
        let seed = params.get_int("seed", 42, 0, 999_999) as u64;
        let frame_key = frame_time_to_key(ctx.frame_time);

        let width = src.width();
        let height = src.height();
        output.resize_clear(width, height)?;

        let inv_w = safe_div_f32(1.0, width as f32, 1.0);
        let inv_h = safe_div_f32(1.0, height as f32, 1.0);
        let aspect = safe_div_f32(width as f32, height as f32, 1.0);
        let src_pixels = src.pixels();
        let w_usize = width as usize;

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let ny = ((y as f32 + 0.5) * inv_h) * 2.0 - 1.0;
            let start = (y as usize).saturating_mul(w_usize);
            if let Some(s_row) = src_pixels.get(start..start + w_usize) {
                for (x, (s, o)) in s_row.iter().zip(out_row.iter_mut()).enumerate() {
                    let nx = (((x as f32 + 0.5) * inv_w) * 2.0 - 1.0) * aspect.min(1.5);
                    let r2 = nx * nx + ny * ny;
                    // Optical cos^4-inspired radial attenuation
                    let angle = (r2.sqrt() * (1.25 - 0.65 * softness)).min(1.2);
                    let cos_a = angle.cos();
                    let cos4 = cos_a * cos_a * cos_a * cos_a;
                    let vig = 1.0 - strength * (1.0 - cos4);

                    let mut r = s.r * vig;
                    let mut g = s.g * vig;
                    let mut b = s.b * vig;

                    if grain > 1e-6 && s.a > 1e-6 {
                        // Midtone-peaked deterministic grain
                        let luma = safe_clamp_f32(s.luma_rec709(), 0.0, 1.0);
                        let envelope = 4.0 * luma * (1.0 - luma) + 0.15;
                        let n =
                            hash_2d_signed(x as i32, y as i32, frame_key, seed) * grain * envelope;
                        r = (r + n).max(0.0);
                        g = (g + n).max(0.0);
                        b = (b + n).max(0.0);
                    }

                    *o = Rgba32F::new(r, g, b, s.a);
                }
            }
        })
    }
}

/// Returns static descriptors for every plugin in the Dulcia Nitentia suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Dulcia Nitentia suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(OpticalVignetteEffect)]
}

bulldog_ofx::register_ofx_suite!((
    DULCIA_OPTICAL_VIGNETTE,
    c"br.com.bulldog.complements.dulcia_nitentia.optical_vignette",
    1,
    0,
    || Box::new(OpticalVignetteEffect)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, RectD, RectI};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn vignette_determinism_and_corner_falloff() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = OpticalVignetteEffect;
        let mut pool = BufferPool::new();
        let mut src = pool.acquire(32, 32).unwrap();
        for px in src.pixels_mut() {
            *px = Rgba32F::new(0.5, 0.5, 0.5, 1.0);
        }
        let params = ParamSet::defaults_from(&effect.descriptor().params);
        let mk_ctx = |t: f64| RenderContext {
            frame_time: t,
            render_scale: [1.0, 1.0],
            bounds: RectI::new(0, 0, 32, 32),
            rod: RectD::new(0.0, 0.0, 32.0, 32.0),
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

        let mut f42_a = pool.acquire(32, 32).unwrap();
        let mut f10 = pool.acquire(32, 32).unwrap();
        let mut f42_b = pool.acquire(32, 32).unwrap();

        effect
            .render(&inputs, &mut f42_a, &params, &mk_ctx(42.0), &mut pool)
            .unwrap();
        effect
            .render(&inputs, &mut f10, &params, &mk_ctx(10.0), &mut pool)
            .unwrap();
        effect
            .render(&inputs, &mut f42_b, &params, &mk_ctx(42.0), &mut pool)
            .unwrap();

        // Random timeline scrubbing produces bit-identical results
        assert_eq!(f42_a, f42_b);
        assert_ne!(f42_a, f10);
        // Corner is darker than center
        assert!(f42_a.get_pixel(0, 0).unwrap().r < f42_a.get_pixel(16, 16).unwrap().r);
    }
}
