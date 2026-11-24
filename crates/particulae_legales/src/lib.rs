//! **Bulldog Particulae Legales** — Stateless deterministic procedural particle systems.

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
    BufferPool, CoreError, CoreResult, DeterministicRng, EffectContext, EffectDescriptor,
    EffectPlugin, IdentityClip, LinearFrame, ParamDescriptor, ParamSet, RenderContext,
    RenderInputs, Rgba32F, render_rows_parallel, safe_clamp_f32, safe_div_f32, sanitize_f64,
};

/// OpenFX identifier for the Particulae Legales Ambient Embers effect.
pub const AMBIENT_EMBERS_ID: &str = "br.com.bulldog.complements.particulae_legales.ambient_embers";

/// Hard upper bound on simulated particles per frame to guarantee bounded render cost (§0.5).
pub const MAX_EMBERS_COUNT: i32 = 256;

#[derive(Clone, Copy)]
struct Ember {
    cx: f32,
    cy: f32,
    inv_radius: f32,
    brightness: f32,
}

/// Stateless deterministic glowing particle field evaluated analytically from
/// `(frame_time, seed)` for exact random-access scrubbing on any NLE timeline (§3.5 / Rule 5).
#[derive(Debug, Default)]
pub struct AmbientEmbersEffect;

impl EffectPlugin for AmbientEmbersEffect {
    fn descriptor(&self) -> EffectDescriptor {
        EffectDescriptor {
            identifier: AMBIENT_EMBERS_ID.to_owned(),
            label: "Bulldog Ambient Embers".to_owned(),
            grouping: "Bulldog Particulae Legales".to_owned(),
            description:
                "Deterministic stateless drifting glowing embers composited in linear light."
                    .to_owned(),
            version_major: 1,
            version_minor: 0,
            context: EffectContext::Filter,
            params: vec![
                ParamDescriptor::double(
                    "density",
                    "Density",
                    "Particle opacity and emission strength",
                    0.4,
                    0.0,
                    1.0,
                ),
                ParamDescriptor::int(
                    "max_particles",
                    "Particle Count",
                    "Maximum active procedural particles (bounded for safety)",
                    48,
                    1,
                    MAX_EMBERS_COUNT,
                ),
                ParamDescriptor::double(
                    "size",
                    "Particle Size",
                    "Normalized radius of particles relative to frame height",
                    0.012,
                    0.002,
                    0.05,
                ),
                ParamDescriptor::double(
                    "speed",
                    "Drift Speed",
                    "Upward drift velocity per timeline second/unit",
                    0.35,
                    0.0,
                    2.0,
                ),
                ParamDescriptor::rgba(
                    "color",
                    "Ember Color",
                    "Linear emission tint of particles",
                    [1.0, 0.55, 0.18, 1.0],
                ),
                ParamDescriptor::int(
                    "seed",
                    "Seed",
                    "Deterministic seed for particle birth positions and phases",
                    101,
                    0,
                    999_999,
                ),
            ],
        }
    }

    fn is_identity(&self, params: &ParamSet, _frame_time: f64) -> Option<IdentityClip> {
        let density = params.get_double("density", 0.4, 0.0, 1.0);
        if density <= 1e-6 {
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
        let density = params.get_double("density", 0.4, 0.0, 1.0) as f32;
        if density <= 1e-6 {
            return output.copy_from(src);
        }

        let count = params.get_int("max_particles", 48, 1, MAX_EMBERS_COUNT) as usize;
        let size = params.get_double("size", 0.012, 0.002, 0.05) as f32;
        let speed = params.get_double("speed", 0.35, 0.0, 2.0) as f32;
        let [cr, cg, cb, _ca] = params.get_rgba("color", [1.0, 0.55, 0.18, 1.0]);
        let seed = params.get_int("seed", 101, 0, 999_999) as u64;

        let t = (sanitize_f64(ctx.frame_time, 0.0) * 0.04) as f32;
        let width = src.width();
        let height = src.height();
        output.resize_clear(width, height)?;

        let aspect = safe_div_f32(width as f32, height as f32, 1.0);
        let inv_w = safe_div_f32(1.0, width as f32, 1.0);
        let inv_h = safe_div_f32(1.0, height as f32, 1.0);

        // Generate bounded deterministic particle array once per frame (<= 256 entries).
        let mut embers = Vec::with_capacity(count);
        for idx in 0..count {
            let mut rng = DeterministicRng::new(0, seed, idx as u64 + 1);
            let x0 = rng.next_f32() * aspect;
            let y0 = rng.next_f32();
            let sway_amp = rng.range_f32(0.01, 0.06);
            let sway_freq = rng.range_f32(0.8, 2.4);
            let phase = rng.next_f32() * TAU;
            let vel = rng.range_f32(0.5, 1.5) * speed;
            let rad = size * rng.range_f32(0.55, 1.45);
            let flicker = 0.65 + 0.35 * (t * sway_freq * 2.5 + phase).sin();

            let cy = (y0 + t * vel).rem_euclid(1.0);
            let cx = (x0 + (t * sway_freq + phase).sin() * sway_amp).rem_euclid(aspect);
            embers.push(Ember {
                cx,
                cy,
                inv_radius: safe_div_f32(1.0, rad, 100.0),
                brightness: density * flicker,
            });
        }

        let src_pixels = src.pixels();
        let w_usize = width as usize;

        render_rows_parallel(output, ctx.abort, |y, out_row| {
            let py = (y as f32 + 0.5) * inv_h;
            let start = (y as usize).saturating_mul(w_usize);
            if let Some(s_row) = src_pixels.get(start..start + w_usize) {
                for (x, (s, o)) in s_row.iter().zip(out_row.iter_mut()).enumerate() {
                    let px = (x as f32 + 0.5) * inv_w * aspect;
                    let mut emit = 0.0f32;
                    for ember in &embers {
                        let dy = (py - ember.cy) * ember.inv_radius;
                        if dy.abs() >= 1.0 {
                            continue;
                        }
                        let dx = (px - ember.cx) * ember.inv_radius;
                        let d2 = dx * dx + dy * dy;
                        if d2 < 1.0 {
                            let falloff = 1.0 - d2;
                            emit += falloff * falloff * ember.brightness;
                        }
                    }
                    let out_a = safe_clamp_f32(s.a + emit.min(1.0) * (1.0 - s.a), 0.0, 1.0);
                    *o = Rgba32F::new(s.r + cr * emit, s.g + cg * emit, s.b + cb * emit, out_a);
                }
            }
        })
    }
}

/// Returns static descriptors for every plugin in the Particulae Legales suite.
#[must_use]
pub fn suite_descriptors() -> Vec<EffectDescriptor> {
    suite_plugins()
        .into_iter()
        .map(|p| p.descriptor())
        .collect()
}

/// Instantiates every plugin in the Particulae Legales suite.
#[must_use]
pub fn suite_plugins() -> Vec<Box<dyn EffectPlugin>> {
    vec![Box::new(AmbientEmbersEffect)]
}

bulldog_ofx::register_ofx_suite!((
    PARTICULAE_AMBIENT_EMBERS,
    c"br.com.bulldog.complements.particulae_legales.ambient_embers",
    1,
    0,
    || Box::new(AmbientEmbersEffect)
),);

#[cfg(test)]
mod tests {
    use bulldog_core::{BackendKind, NeverAbort, ParamValue, RectD, RectI};
    use bulldog_ofx::validate_suite;

    use super::*;

    #[test]
    fn embers_random_access_determinism_and_particle_bound() {
        assert!(validate_suite(&suite_descriptors()).is_ok());

        let effect = AmbientEmbersEffect;
        let mut pool = BufferPool::new();
        let mut src = pool.acquire(32, 32).unwrap();
        for px in src.pixels_mut() {
            *px = Rgba32F::BLACK;
        }

        let mut params = ParamSet::defaults_from(&effect.descriptor().params);
        // Hostile particle count far above ceiling must be clamped cleanly.
        params.insert("max_particles", ParamValue::Int(10_000_000));
        params.insert("density", ParamValue::Double(0.9));

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

        let mut f100_a = pool.acquire(32, 32).unwrap();
        let mut f5 = pool.acquire(32, 32).unwrap();
        let mut f100_b = pool.acquire(32, 32).unwrap();
        effect
            .render(&inputs, &mut f100_a, &params, &mk_ctx(100.0), &mut pool)
            .unwrap();
        effect
            .render(&inputs, &mut f5, &params, &mk_ctx(5.0), &mut pool)
            .unwrap();
        effect
            .render(&inputs, &mut f100_b, &params, &mk_ctx(100.0), &mut pool)
            .unwrap();

        assert_eq!(f100_a, f100_b);
        assert_ne!(f100_a, f5);
    }
}
