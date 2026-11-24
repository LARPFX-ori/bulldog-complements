//! **Bulldog TrackerMan** — Planar tracking, projective transforms, stabilization, and surface inserts.

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
    render_rows_parallel, safe_div_f32,
};

/// OpenFX identifier for the TrackerMan Planar Transform effect.
pub const PLANAR_TRANSFORM_ID: &str = "br.com.bulldog.complements.tracker_man.planar_transform";

/// Resolution-independent planar affine + projective tilt transform with bilinear sampling.
#[derive(Debug, Default)]
pub struct PlanarTransformEffect;

impl EffectPlugin for PlanarTransformEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: PLANAR_TRANSFORM_ID.to_owned(),
            label: "Bulldog Planar Transform".to_owned(),
            grouping: "Bulldog TrackerMan".to_owned(),
            description: "Resolution-independent planar translation, scale, rotation, and perspective tilt warp.".to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::point2d(
                    "center",
                    "Pivot Center",
                    "Normalized transform pivot point",
                    [0.5, 0.5],
                    [-1.0, -1.0],
                    [2.0, 2.0],
                ),
                ParamDescriptor::point2d(
                    "translate",
                    "Translate",
                    "Normalized horizontal and vertical offset",
                    [0.0, 0.0],
                    [-1.0, -1.0],
                    [1.0, 1.0],
                ),
                ParamDescriptor::double(
                    "scale",
                    "Scale",
                    "Uniform planar scale factor",
                    1.0,
                    0.05,
                    8.0,
                ),
                ParamDescriptor::double(
                    "rotation_deg",
                    "Rotation",
                    "Planar rotation angle in degrees",
                    0.0,
                    -360.0,
                    360.0,
                ),
                ParamDescriptor::double(
                    "tilt_x",
                    "Perspective Tilt X",
                    "Horizontal projective keystone tilt",
                    0.0,
                    -0.6,
                    0.6,
                ),
                ParamDescriptor::double(
                    "tilt_y",
                    "Perspective Tilt Y",
                    "Vertical projective keystone tilt",
                    0.0,
                    -0.6,
                    0.6,
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let [tx, ty] = params.get_point2d("translate", [0.0, 0.0], [-1.0, -1.0], [1.0, 1.0]);
        let scale = params.get_double("scale", 1.0, 0.05, 8.0);
        let rot = params.get_double("rotation_deg", 0.0, -360.0, 360.0);
        let tilt_x = params.get_double("tilt_x", 0.0, -0.6, 0.6);
        let tilt_y = params.get_double("tilt_y", 0.0, -0.6, 0.6);

        if tx.abs() <= 1e-6
            && ty.abs() <= 1e-6
            && (scale - 1.0).abs() <= 1e-6
            && rot.abs() <= 1e-6
            && tilt_x.abs() <= 1e-6
            && tilt_y.abs() <= 1e-6
        {
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
        let [cx, cy] = params.get_point2d("center", [0.5, 0.5], [-1.0, -1.0], [2.0, 2.0]);
        let [tx, ty] = params.get_point2d("translate", [0.0, 0.0], [-1.0, -1.0], [1.0, 1.0]);
        let scale = params.get_double("scale", 1.0, 0.05, 8.0) as f32;
        let rot_rad = (params.get_double("rotation_deg", 0.0, -360.0, 360.0) as f32).to_radians();
        let tilt_x = params.get_double("tilt_x", 0.0, -0.6, 0.6) as f32;
        let tilt_y = params.get_double("tilt_y", 0.0, -0.6, 0.6) as f32;

        let width = output.width();
        let height = output.height();
        let inv_w = safe_div_f32(1.0, width as f32, 1.0);
        let inv_h = safe_div_f32(1.0, height as f32, 1.0);
        let aspect = safe_div_f32(width as f32, height as f32, 1.0);
        let inv_aspect = safe_div_f32(1.0, aspect, 1.0);
        let inv_scale = safe_div_f32(1.0, scale, 1.0);
        let (sin_r, cos_r) = (-rot_rad).sin_cos();

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let v = (y as f32 + 0.5) * inv_h;
            for (x, out_px) in out_row.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) * inv_w;
                let dx = (u - cx - tx) * aspect;
                let dy = v - cy - ty;

                // Inverse projective divisor
                let w_proj = 1.0 + dx * tilt_x + dy * tilt_y;
                if w_proj <= 1e-4 {
                    *out_px = Rgba32F::TRANSPARENT;
                    continue;
                }
                let px = (dx / w_proj) * inv_scale;
                let py = (dy / w_proj) * inv_scale;

                let rx = px * cos_r - py * sin_r;
                let ry = px * sin_r + py * cos_r;

                let src_u = rx * inv_aspect + cx;
                let src_v = ry + cy;
                *out_px = src.sample_bilinear(src_u, src_v, false);
            }
        })
    }
}

/// Returns static descriptors for every plugin in the TrackerMan suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the TrackerMan suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(PlanarTransformEffect)]
}

bulldog_ofx::register_ofx_suite!((
    TRACKER_MAN_PLANAR_TRANSFORM,
    c"br.com.bulldog.complements.tracker_man.planar_transform",
    1,
    0,
    || Box::new(PlanarTransformEffect)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, ParamValue, RectD, RectI};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn planar_transform_identity_and_translation() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = PlanarTransformEffect;
        let defaults = ParamSet::defaults_from(&effect.descriptor().params);
        assert_eq!(
            effect.is_identity(&defaults, 0.0),
            Some(IdentityClip::Source)
        );

        let mut pool = BufferPool::new();
        let mut src = pool.acquire(16, 16).unwrap();
        for px in src.pixels_mut() {
            *px = Rgba32F::WHITE;
        }
        let mut out = pool.acquire(16, 16).unwrap();
        let mut shifted = defaults;
        shifted.insert("translate", ParamValue::Point2D([0.5, 0.0]));
        assert!(effect.is_identity(&shifted, 0.0).is_none());

        let ctx = RenderContext {
            frame_time: 0.0,
            render_scale: [1.0, 1.0],
            bounds: RectI::new(0, 0, 16, 16),
            rod: RectD::new(0.0, 0.0, 16.0, 16.0),
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
            .render(&inputs, &mut out, &shifted, &ctx, &mut pool)
            .unwrap();

        // Left half shifted right -> transparent border on x = 0, white on x = 12
        assert_eq!(out.get_pixel(0, 8).unwrap().a, 0.0);
        assert!(out.get_pixel(12, 8).unwrap().r > 0.95);
    }
}
