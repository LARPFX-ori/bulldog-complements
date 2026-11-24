//! **Bulldog Continuus** — General-purpose finishing, blur, stylize, and compositing suite.

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
    LinearFrame, ParamDescriptor, ParamSet, RectD, RenderContext, RenderInputs,
    normalized_radius_to_pixels, render_rows_parallel,
};

/// OpenFX identifier for the Continuus Soft Blur effect.
pub const SOFT_BLUR_ID: &str = "br.com.bulldog.complements.continuus.soft_blur";

/// Resolution-independent linear-light Gaussian approximation blur with RoD/RoI expansion.
#[derive(Debug, Default)]
pub struct SoftBlurEffect;

impl SoftBlurEffect {
    fn radius_padding(rect: RectD, params: &ParamSet) -> f64 {
        let norm_r = params.get_double("radius", 0.015, 0.0, 0.25);
        let mix = params.get_double("mix", 1.0, 0.0, 1.0);
        if norm_r <= 1e-6 || mix <= 1e-6 {
            return 0.0;
        }
        let ref_dim = rect.width().min(rect.height()).max(1.0);
        (norm_r * ref_dim * 3.0).min(f64::from(MAX_BLUR_RADIUS_PX) * 3.0)
    }
}

impl EffectPlugin for SoftBlurEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: SOFT_BLUR_ID.to_owned(),
            label: "Bulldog Soft Blur".to_owned(),
            grouping: "Bulldog Continuus".to_owned(),
            description: "Resolution-independent multi-pass linear-light blur with premultiplied alpha preservation.".to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::double(
                    "radius",
                    "Radius",
                    "Blur radius normalized to the shorter frame dimension",
                    0.015,
                    0.0,
                    0.25,
                ),
                ParamDescriptor::double(
                    "mix",
                    "Mix",
                    "Blend between original source (0.0) and blurred result (1.0)",
                    1.0,
                    0.0,
                    1.0,
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let radius = params.get_double("radius", 0.015, 0.0, 0.25);
        let mix = params.get_double("mix", 1.0, 0.0, 1.0);
        if radius <= 1e-6 || mix <= 1e-6 {
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
        let radius_norm = params.get_double("radius", 0.015, 0.0, 0.25);
        let mix = params.get_double("mix", 1.0, 0.0, 1.0) as f32;
        let radius_px =
            normalized_radius_to_pixels(radius_norm, src.width(), src.height(), MAX_BLUR_RADIUS_PX);

        if radius_px == 0 || mix <= 1e-6 {
            return output.copy_from(src);
        }

        let mut scratch = pool.acquire(src.width(), src.height())?;
        let res = gaussian_approx_blur(src, output, &mut scratch, radius_px, ctx.abort);
        pool.release(scratch);
        res?;

        if mix < 1.0 - 1e-6 {
            let width = output.width() as usize;
            let src_pixels = src.pixels();
            render_rows_parallel(output, ctx.abort, |y, row| {
                let start = (y as usize).saturating_mul(width);
                if let Some(s_row) = src_pixels.get(start..start + width) {
                    for (s, o) in s_row.iter().zip(row.iter_mut()) {
                        *o = s.lerp(*o, mix);
                    }
                }
            })?;
        }

        Ok(())
    }
}

/// Returns static descriptors for every plugin in the Continuus suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Continuus suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(SoftBlurEffect)]
}

bulldog_ofx::register_ofx_suite!((
    CONTINUUS_SOFT_BLUR,
    c"br.com.bulldog.complements.continuus.soft_blur",
    1,
    0,
    || Box::new(SoftBlurEffect)
),);

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use std::collections::BTreeMap;

    use bulldog_core::{EffectContext, ParamValue, PixelFormat, PreMultiplication};
    use bulldog_ofx::mock_host::{MockImageSpec, MockLifecycleRequest, run_mock_lifecycle};
    use bulldog_ofx::{K_OFX_STAT_OK, validate_suite};

    use super::*;

    #[test]
    fn continuus_descriptors_are_valid() {
        assert!(validate_suite(&suite_descriptors()).is_ok());
        assert_eq!(ofx_plugin_count(), 1);
        assert!(!ofx_plugin(0).is_null());
        assert!(ofx_plugin(-1).is_null());
        assert!(ofx_plugin(1).is_null());
    }

    #[test]
    fn soft_blur_full_ofx_lifecycle_and_hostile_params() {
        let w = 32u32;
        let h = 32u32;
        let mut src = vec![0u8; (w * h * 4) as usize];
        // Bright white impulse at center
        let center = ((16 * w + 16) * 4) as usize;
        src[center..center + 4].copy_from_slice(&[255, 255, 255, 255]);

        let mut clips = BTreeMap::new();
        clips.insert(
            "Source".to_owned(),
            MockImageSpec::tightly_packed(
                src.clone(),
                w,
                h,
                PixelFormat::Rgba8,
                PreMultiplication::UnPreMultiplied,
            ),
        );
        clips.insert(
            "Output".to_owned(),
            MockImageSpec::tightly_packed(
                vec![0u8; (w * h * 4) as usize],
                w,
                h,
                PixelFormat::Rgba8,
                PreMultiplication::UnPreMultiplied,
            ),
        );

        let mut overrides = BTreeMap::new();
        overrides.insert("radius".to_owned(), ParamValue::Double(0.1));
        overrides.insert("mix".to_owned(), ParamValue::Double(1.0));

        // SAFETY: `OfxGetPlugin(0)` returns a static exported `OfxPlugin`.
        let report = unsafe {
            run_mock_lifecycle(
                ofx_plugin(0),
                MockLifecycleRequest {
                    context: EffectContext::Filter,
                    time: 10.0,
                    clips,
                    param_overrides: overrides,
                    aborted: false,
                },
            )
        }
        .unwrap();

        assert_eq!(report.load_status, K_OFX_STAT_OK);
        assert_eq!(report.describe_status, K_OFX_STAT_OK);
        assert_eq!(report.render_status, K_OFX_STAT_OK);
        assert_eq!(report.destroy_status, K_OFX_STAT_OK);
        assert_eq!(report.released_images, 2);
        // RoD must expand when radius > 0
        assert!(report.rod.x1 < 0.0 && report.rod.x2 > f64::from(w));
        // Neighbor pixel next to impulse must now carry diffused energy
        let neighbor = ((16 * w + 17) * 4) as usize;
        assert!(report.output_bytes[neighbor] > 0);

        // Identity + NaN resilience check
        let mut id_clips = BTreeMap::new();
        id_clips.insert(
            "Source".to_owned(),
            MockImageSpec::tightly_packed(
                src.clone(),
                w,
                h,
                PixelFormat::Rgba8,
                PreMultiplication::UnPreMultiplied,
            ),
        );
        id_clips.insert(
            "Output".to_owned(),
            MockImageSpec::tightly_packed(
                vec![0u8; (w * h * 4) as usize],
                w,
                h,
                PixelFormat::Rgba8,
                PreMultiplication::UnPreMultiplied,
            ),
        );
        let mut id_overrides = BTreeMap::new();
        id_overrides.insert("radius".to_owned(), ParamValue::Double(0.0));
        id_overrides.insert("mix".to_owned(), ParamValue::Double(f64::NAN));
        let id_report = unsafe {
            run_mock_lifecycle(
                ofx_plugin(0),
                MockLifecycleRequest {
                    context: EffectContext::Filter,
                    time: 10.0,
                    clips: id_clips,
                    param_overrides: id_overrides,
                    aborted: false,
                },
            )
        }
        .unwrap();
        assert_eq!(id_report.is_identity_clip.as_deref(), Some("Source"));
        assert_eq!(id_report.output_bytes, src);
    }
}
