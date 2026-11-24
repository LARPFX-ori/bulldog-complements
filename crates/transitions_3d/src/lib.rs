//! **Bulldog 3D Transitions** — True perspective 3D spatial transitions operating in
//! `OfxImageEffectContextTransition` with normalized `Transition` progress (`0.0..=1.0`).

#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use core::f32::consts::FRAC_PI_2;

use bulldog_core::{
    BufferPool, CoreError, CoreResult, EffectContext, EffectDescriptor, EffectPlugin, IdentityClip,
    LinearFrame, ParamDescriptor, ParamSet, RenderContext, RenderInputs, Rgba32F,
    render_rows_parallel, safe_clamp_f32, safe_div_f32,
};

/// OpenFX identifier for the 3D Transitions Cube Spin transition.
pub const CUBE_SPIN_ID: &str = "br.com.bulldog.complements.transitions_3d.cube_spin";

/// Analytic perspective 3D cube rotation transition between `SourceFrom` and `SourceTo`
/// with directional shading and smooth edge depth sorting.
#[derive(Debug, Default)]
pub struct CubeSpinTransition;

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    fn add_scaled(self, other: Self, s: f32) -> Self {
        Self::new(
            self.x + other.x * s,
            self.y + other.y * s,
            self.z + other.z * s,
        )
    }

    fn rot_y(self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(self.x * c + self.z * s, self.y, -self.x * s + self.z * c)
    }

    fn rot_x(self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::new(self.x, self.y * c - self.z * s, self.y * s + self.z * c)
    }
}

struct CubeFace {
    center: Vec3,
    u_axis: Vec3,
    v_axis: Vec3,
    normal: Vec3,
}

impl CubeFace {
    fn intersect_ray(&self, origin: Vec3, dir: Vec3) -> Option<(f32, f32, f32)> {
        let denom = self.normal.dot(dir);
        // Backface cull & parallel guard
        if denom >= -1e-5 {
            return None;
        }
        let t = self.center.sub(origin).dot(self.normal) / denom;
        if t <= 0.0 || !t.is_finite() {
            return None;
        }
        let hit = origin.add_scaled(dir, t);
        let rel = hit.sub(self.center);
        let u = rel.dot(self.u_axis) * 0.5 + 0.5;
        let v = rel.dot(self.v_axis) * 0.5 + 0.5;
        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
            Some((t, u, v))
        } else {
            None
        }
    }
}

impl EffectPlugin for CubeSpinTransition {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: CUBE_SPIN_ID.to_owned(),
            label: "Bulldog 3D Cube Spin".to_owned(),
            grouping: "Bulldog 3D Transitions".to_owned(),
            description: "Perspective 3D rotating cube transition between outgoing and incoming clips with directional shading.".to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Transition,
            params: vec![
                ParamDescriptor::double(
                    "Transition",
                    "Transition",
                    "Normalized transition progress from outgoing (0.0) to incoming (1.0) clip",
                    0.0,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::choice(
                    "direction",
                    "Direction",
                    "Axis and sense of 3D cube rotation",
                    0,
                    &["Left", "Right", "Up", "Down"],
                ),
                ParamDescriptor::double(
                    "perspective",
                    "Perspective",
                    "Camera perspective depth factor",
                    0.55,
                    0.1,
                    1.5,
                ),
                ParamDescriptor::double(
                    "lighting",
                    "Lighting",
                    "Directional shading strength on angled faces",
                    0.35,
                    0.0,
                    1.0,
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

        let direction = params.get_choice("direction", 0, 4);
        let perspective = params.get_double("perspective", 0.55, 0.1, 1.5) as f32;
        let lighting = params.get_double("lighting", 0.35, 0.0, 1.0) as f32;

        // Smooth ease-in-out curve for cinematic inertia
        let eased = progress * progress * (3.0 - 2.0 * progress);
        let angle = eased * FRAC_PI_2;

        // Place camera along +Z looking towards origin; unit cube spans [-1, 1]^3 rotated around origin.
        let focal = safe_div_f32(1.0, perspective, 1.8);
        let cam_z = focal + 1.0;
        let cam_origin = Vec3::new(0.0, 0.0, cam_z);

        let transform = |v: Vec3| -> Vec3 {
            match direction {
                0 => v.rot_y(-angle), // Left
                1 => v.rot_y(angle),  // Right
                2 => v.rot_x(angle),  // Up
                _ => v.rot_x(-angle), // Down
            }
        };

        // Face A (SourceFrom): starts on +Z face (z = +1).
        let face_from = CubeFace {
            center: transform(Vec3::new(0.0, 0.0, 1.0)),
            u_axis: transform(Vec3::new(1.0, 0.0, 0.0)),
            v_axis: transform(Vec3::new(0.0, 1.0, 0.0)),
            normal: transform(Vec3::new(0.0, 0.0, 1.0)),
        };

        // Face B (SourceTo): orthogonal adjacent face rotating into view.
        let (b_center, b_u, b_v, b_n) = match direction {
            0 => (
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ),
            1 => (
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
            ),
            2 => (
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, -1.0, 0.0),
            ),
            _ => (
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
        };
        let face_to = CubeFace {
            center: transform(b_center),
            u_axis: transform(b_u),
            v_axis: transform(b_v),
            normal: transform(b_n),
        };

        let light_dir = Vec3::new(0.25, 0.35, 0.90);
        let light_len = light_dir.dot(light_dir).sqrt().max(1e-5);
        let light_n = Vec3::new(
            light_dir.x / light_len,
            light_dir.y / light_len,
            light_dir.z / light_len,
        );

        let shade_from =
            1.0 - lighting * (1.0 - safe_clamp_f32(face_from.normal.dot(light_n), 0.15, 1.0));
        let shade_to =
            1.0 - lighting * (1.0 - safe_clamp_f32(face_to.normal.dot(light_n), 0.15, 1.0));

        let width = output.width();
        let height = output.height();
        let inv_w = safe_div_f32(1.0, width as f32, 1.0);
        let inv_h = safe_div_f32(1.0, height as f32, 1.0);

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let sy = ((y as f32 + 0.5) * inv_h) * 2.0 - 1.0;
            for (x, out_px) in out_row.iter_mut().enumerate() {
                let sx = ((x as f32 + 0.5) * inv_w) * 2.0 - 1.0;
                let ray_dir = Vec3::new(sx, sy, -focal);

                let hit_a = face_from.intersect_ray(cam_origin, ray_dir);
                let hit_b = face_to.intersect_ray(cam_origin, ray_dir);

                *out_px = match (hit_a, hit_b) {
                    (Some((ta, ua, va)), Some((tb, ub, vb))) => {
                        if ta <= tb {
                            from_img.sample_bilinear(ua, va, true).scale_rgb(shade_from)
                        } else {
                            to_img.sample_bilinear(ub, vb, true).scale_rgb(shade_to)
                        }
                    }
                    (Some((_, ua, va)), None) => {
                        from_img.sample_bilinear(ua, va, true).scale_rgb(shade_from)
                    }
                    (None, Some((_, ub, vb))) => {
                        to_img.sample_bilinear(ub, vb, true).scale_rgb(shade_to)
                    }
                    (None, None) => Rgba32F::TRANSPARENT,
                };
            }
        })
    }
}

/// Returns static descriptors for every plugin in the 3D Transitions suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the 3D Transitions suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(CubeSpinTransition)]
}

bulldog_ofx::register_ofx_suite!((
    TRANSITIONS_3D_CUBE_SPIN,
    c"br.com.bulldog.complements.transitions_3d.cube_spin",
    1,
    0,
    || Box::new(CubeSpinTransition)
),);

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use std::collections::BTreeMap;

    use bulldog_core::{ParamValue, PixelFormat, PreMultiplication};
    use bulldog_ofx::mock_host::{MockImageSpec, MockLifecycleRequest, run_mock_lifecycle};
    use bulldog_ofx::{K_OFX_STAT_OK, validate_suite};

    use super::*;

    #[test]
    fn cube_spin_ofx_transition_lifecycle_and_identity_endpoints() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let w = 32u32;
        let h = 32u32;
        let mut red = vec![0u8; (w * h * 4) as usize];
        let mut blue = vec![0u8; (w * h * 4) as usize];
        for px in red.chunks_exact_mut(4) {
            px.copy_from_slice(&[240, 20, 20, 255]);
        }
        for px in blue.chunks_exact_mut(4) {
            px.copy_from_slice(&[20, 40, 240, 255]);
        }

        let run_at = |progress: f64| {
            let mut clips = BTreeMap::new();
            clips.insert(
                "SourceFrom".to_owned(),
                MockImageSpec::tightly_packed(
                    red.clone(),
                    w,
                    h,
                    PixelFormat::Rgba8,
                    PreMultiplication::Opaque,
                ),
            );
            clips.insert(
                "SourceTo".to_owned(),
                MockImageSpec::tightly_packed(
                    blue.clone(),
                    w,
                    h,
                    PixelFormat::Rgba8,
                    PreMultiplication::Opaque,
                ),
            );
            clips.insert(
                "Output".to_owned(),
                MockImageSpec::tightly_packed(
                    vec![0u8; (w * h * 4) as usize],
                    w,
                    h,
                    PixelFormat::Rgba8,
                    PreMultiplication::Opaque,
                ),
            );
            let mut overrides = BTreeMap::new();
            overrides.insert("Transition".to_owned(), ParamValue::Double(progress));
            // SAFETY: `OfxGetPlugin(0)` returns a valid static descriptor.
            unsafe {
                run_mock_lifecycle(
                    ofx_plugin(0),
                    MockLifecycleRequest {
                        context: EffectContext::Transition,
                        time: progress * 30.0,
                        clips,
                        param_overrides: overrides,
                        aborted: false,
                    },
                )
            }
            .unwrap()
        };

        let r0 = run_at(0.0);
        assert_eq!(r0.render_status, K_OFX_STAT_OK);
        assert_eq!(r0.is_identity_clip.as_deref(), Some("SourceFrom"));
        assert_eq!(r0.output_bytes, red);

        let r1 = run_at(1.0);
        assert_eq!(r1.render_status, K_OFX_STAT_OK);
        assert_eq!(r1.is_identity_clip.as_deref(), Some("SourceTo"));
        assert_eq!(r1.output_bytes, blue);

        // At midpoint (0.5), left side shows outgoing red face and right side shows incoming blue face
        let r_mid = run_at(0.5);
        assert_eq!(r_mid.render_status, K_OFX_STAT_OK);
        assert!(r_mid.is_identity_clip.is_none());
        let left_px = ((16 * w + 8) * 4) as usize;
        let right_px = ((16 * w + 24) * 4) as usize;
        assert!(r_mid.output_bytes[left_px] > r_mid.output_bytes[left_px + 2]);
        assert!(r_mid.output_bytes[right_px + 2] > r_mid.output_bytes[right_px]);
    }
}
