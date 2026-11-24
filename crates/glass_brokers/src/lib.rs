//! **Bulldog Glass Brokers** — 3D faceted geometric transitions, shatters, and crystalline optics.

#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use core::f32::consts::PI;

use bulldog_core::{
    BufferPool, CoreError, CoreResult, EffectContext, EffectDescriptor, EffectPlugin, IdentityClip,
    LinearFrame, ParamDescriptor, ParamSet, RenderContext, RenderInputs, Rgba32F, hash_2d,
    render_rows_parallel, safe_clamp_f32, safe_div_f32,
};

/// OpenFX identifier for the Glass Brokers Facet Wipe 3D transition.
pub const FACET_WIPE_ID: &str = "br.com.bulldog.complements.glass_brokers.facet_wipe";

/// Deterministic 3D faceted tile-flip geometric transition with per-tile stagger
/// and directional specular bevel shading.
#[derive(Debug, Default)]
pub struct FacetWipeTransition;

impl EffectPlugin for FacetWipeTransition {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: FACET_WIPE_ID.to_owned(),
            label: "Bulldog Facet Wipe 3D".to_owned(),
            grouping: "Bulldog Glass Brokers".to_owned(),
            description: "Deterministic 3D rotating tile grid transition with directional facet illumination.".to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Transition,
            params: vec![
                ParamDescriptor::double(
                    "Transition",
                    "Transition",
                    "Normalized transition progress from outgoing (0.0) to incoming (1.0)",
                    0.0,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::int(
                    "columns",
                    "Columns",
                    "Number of horizontal geometric facets",
                    8,
                    2,
                    24,
                ),
                ParamDescriptor::int(
                    "rows",
                    "Rows",
                    "Number of vertical geometric facets",
                    5,
                    2,
                    24,
                ),
                ParamDescriptor::double(
                    "bevel_light",
                    "Facet Lighting",
                    "Directional shading intensity while tiles rotate",
                    0.4,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::int(
                    "seed",
                    "Seed",
                    "Deterministic seed controlling per-facet stagger timing",
                    7,
                    0,
                    999_999,
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let p = params.get_double("Transition", 0.0, 0.0, 1.0);
        if p <= 1e-6 {
            Some(IdentityClip::SourceFrom)
        } else if p >= 1.0 - 1e-6 {
            Some(IdentityClip::SourceTo)
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
        let Some(from_img) = inputs.source_from else {
            return Err(CoreError::MissingInputClip("SourceFrom"));
        };
        let Some(to_img) = inputs.source_to else {
            return Err(CoreError::MissingInputClip("SourceTo"));
        };
        let progress = params.get_double("Transition", 0.0, 0.0, 1.0) as f32;
        if progress <= 1e-6 {
            return output.copy_from(from_img);
        }
        if progress >= 1.0 - 1e-6 {
            return output.copy_from(to_img);
        }

        let cols = params.get_int("columns", 8, 2, 24) as f32;
        let rows = params.get_int("rows", 5, 2, 24) as f32;
        let bevel_light = params.get_double("bevel_light", 0.4, 0.0, 1.0) as f32;
        let seed = params.get_int("seed", 7, 0, 999_999) as u64;

        let width = output.width();
        let height = output.height();
        let inv_w = safe_div_f32(1.0, width as f32, 1.0);
        let inv_h = safe_div_f32(1.0, height as f32, 1.0);

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let v = (y as f32 + 0.5) * inv_h;
            let row_idx = (v * rows).floor().clamp(0.0, rows - 1.0);
            for (x, out_px) in out_row.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) * inv_w;
                let col_idx = (u * cols).floor().clamp(0.0, cols - 1.0);

                // Deterministic tile delay combining diagonal wave + seeded jitter
                let diag = (col_idx / cols + row_idx / rows) * 0.25;
                let jitter = hash_2d(col_idx as i32, row_idx as i32, 0, seed) * 0.2;
                let start_t = diag + jitter;
                let duration = 0.55;
                let local_p = safe_clamp_f32((progress - start_t) / duration, 0.0, 1.0);
                let eased = local_p * local_p * (3.0 - 2.0 * local_p);

                let angle = eased * PI;
                let cos_a = angle.cos();
                let abs_cos = cos_a.abs();

                if abs_cos < 0.02 {
                    *out_px = Rgba32F::TRANSPARENT;
                    continue;
                }

                // Local coordinate inside the tile [-0.5, 0.5]
                let tile_u = u * cols - col_idx - 0.5;
                let foreshortened_u = tile_u / abs_cos;
                if !(-0.5..=0.5).contains(&foreshortened_u) {
                    *out_px = Rgba32F::TRANSPARENT;
                    continue;
                }

                let mapped_u = if cos_a >= 0.0 {
                    (col_idx + 0.5 + foreshortened_u) / cols
                } else {
                    (col_idx + 0.5 - foreshortened_u) / cols
                };

                let sample = if cos_a >= 0.0 {
                    from_img.sample_bilinear(mapped_u, v, true)
                } else {
                    to_img.sample_bilinear(mapped_u, v, true)
                };

                let shade = 1.0 - bevel_light * (1.0 - abs_cos);
                let glint = bevel_light * 0.25 * angle.sin().powi(4);
                *out_px = Rgba32F::new(
                    sample.r * shade + glint,
                    sample.g * shade + glint,
                    sample.b * shade + glint,
                    sample.a,
                );
            }
        })
    }
}

/// Returns static descriptors for every plugin in the Glass Brokers suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Glass Brokers suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(FacetWipeTransition)]
}

bulldog_ofx::register_ofx_suite!((
    GLASS_BROKERS_FACET_WIPE,
    c"br.com.bulldog.complements.glass_brokers.facet_wipe",
    1,
    0,
    || Box::new(FacetWipeTransition)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, ParamValue, RectD, RectI};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn facet_wipe_valid_and_deterministic() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = FacetWipeTransition;
        let mut pool = BufferPool::new();
        let mut a = pool.acquire(24, 24).unwrap();
        let mut b = pool.acquire(24, 24).unwrap();
        for px in a.pixels_mut() {
            *px = Rgba32F::new(1.0, 0.1, 0.1, 1.0);
        }
        for px in b.pixels_mut() {
            *px = Rgba32F::new(0.1, 0.9, 0.2, 1.0);
        }

        let mut params = ParamSet::defaults_from(&effect.descriptor().params);
        params.insert("Transition", ParamValue::Double(0.5));

        let ctx = RenderContext {
            frame_time: 15.0,
            render_scale: [1.0, 1.0],
            bounds: RectI::new(0, 0, 24, 24),
            rod: RectD::new(0.0, 0.0, 24.0, 24.0),
            color_space: bulldog_core::ColorSpace::Linear,
            premult: bulldog_core::PreMultiplication::Opaque,
            backend: BackendKind::Cpu,
            abort: &NeverAbort,
        };
        let inputs = RenderInputs {
            source: None,
            source_from: Some(&a),
            source_to: Some(&b),
        };
        let mut out1 = pool.acquire(24, 24).unwrap();
        let mut out2 = pool.acquire(24, 24).unwrap();
        effect
            .render(&inputs, &mut out1, &params, &ctx, &mut pool)
            .unwrap();
        effect
            .render(&inputs, &mut out2, &params, &ctx, &mut pool)
            .unwrap();
        assert_eq!(out1, out2);
    }
}
