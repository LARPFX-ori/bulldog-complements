//! **Bulldog Caeruleus Nebulosus** — Production utilities, framing mattes, and essential video tools.

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
    render_rows_parallel, safe_clamp_f32, safe_div_f32,
};

/// OpenFX identifier for the Caeruleus Nebulosus Letterbox Matte utility.
pub const LETTERBOX_MATTE_ID: &str =
    "br.com.bulldog.complements.caeruleus_nebulosus.letterbox_matte";

/// Resolution-independent anti-aliased letterbox / aspect-ratio framing matte.
#[derive(Debug, Default)]
pub struct LetterboxMatteEffect;

impl EffectPlugin for LetterboxMatteEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: LETTERBOX_MATTE_ID.to_owned(),
            label: "Bulldog Letterbox Matte".to_owned(),
            grouping: "Bulldog Caeruleus Nebulosus".to_owned(),
            description: "Resolution-independent top and bottom framing matte with feathered edge."
                .to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::double(
                    "bar_height",
                    "Bar Height",
                    "Normalized height of each matte bar (0.0 to 0.45 of frame height)",
                    0.12,
                    0.0,
                    0.45,
                ),
                ParamDescriptor::double(
                    "feather",
                    "Feather",
                    "Normalized edge feathering width",
                    0.002,
                    0.0,
                    0.1,
                ),
                ParamDescriptor::double("opacity", "Opacity", "Matte opacity", 1.0, 0.0, 1.0),
                ParamDescriptor::rgba(
                    "color",
                    "Matte Color",
                    "Linear color of the letterbox bars",
                    [0.0, 0.0, 0.0, 1.0],
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let bar = params.get_double("bar_height", 0.12, 0.0, 0.45);
        let opacity = params.get_double("opacity", 1.0, 0.0, 1.0);
        if bar <= 1e-6 || opacity <= 1e-6 {
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
        let bar = params.get_double("bar_height", 0.12, 0.0, 0.45) as f32;
        let feather = params.get_double("feather", 0.002, 0.0, 0.1) as f32;
        let opacity = params.get_double("opacity", 1.0, 0.0, 1.0) as f32;
        let [cr, cg, cb, ca] = params.get_rgba("color", [0.0, 0.0, 0.0, 1.0]);
        let matte_color = Rgba32F::new(cr, cg, cb, ca);

        let width = src.width();
        let height = src.height();
        output.resize_clear(width, height)?;

        let inv_h = safe_div_f32(1.0, height as f32, 1.0);
        let src_pixels = src.pixels();
        let w_usize = width as usize;

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let v = (y as f32 + 0.5) * inv_h;
            let dist_from_edge = v.min(1.0 - v);
            let mask = if feather <= 1e-6 {
                if dist_from_edge < bar { 1.0 } else { 0.0 }
            } else {
                let t = safe_clamp_f32((bar - dist_from_edge) / feather + 0.5, 0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            };
            let blend = safe_clamp_f32(mask * opacity * matte_color.a, 0.0, 1.0);

            let start = (y as usize).saturating_mul(w_usize);
            if let Some(s_row) = src_pixels.get(start..start + w_usize) {
                for (s, o) in s_row.iter().zip(out_row.iter_mut()) {
                    if blend <= 1e-6 {
                        *o = *s;
                    } else {
                        let fg = Rgba32F::new(matte_color.r, matte_color.g, matte_color.b, blend);
                        *o = fg.over(*s);
                    }
                }
            }
        })
    }
}

/// Returns static descriptors for every plugin in the Caeruleus Nebulosus suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Caeruleus Nebulosus suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(LetterboxMatteEffect)]
}

bulldog_ofx::register_ofx_suite!((
    CAERULEUS_LETTERBOX_MATTE,
    c"br.com.bulldog.complements.caeruleus_nebulosus.letterbox_matte",
    1,
    0,
    || Box::new(LetterboxMatteEffect)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, RectD, RectI};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn letterbox_masks_edges_and_preserves_center() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = LetterboxMatteEffect;
        let mut pool = BufferPool::new();
        let mut src = pool.acquire(20, 20).unwrap();
        for px in src.pixels_mut() {
            *px = Rgba32F::WHITE;
        }
        let mut out = pool.acquire(20, 20).unwrap();
        let params = ParamSet::defaults_from(&effect.descriptor().params);
        let ctx = RenderContext {
            frame_time: 0.0,
            render_scale: [1.0, 1.0],
            bounds: RectI::new(0, 0, 20, 20),
            rod: RectD::new(0.0, 0.0, 20.0, 20.0),
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
        effect
            .render(&inputs, &mut out, &params, &ctx, &mut pool)
            .unwrap();

        assert!(out.get_pixel(10, 0).unwrap().r < 0.05);
        assert!(out.get_pixel(10, 10).unwrap().r > 0.99);
        assert!(out.get_pixel(10, 19).unwrap().r < 0.05);
    }
}
