use crate::math::{safe_clamp_f32, sanitize_f32};

/// Transfer function / color encoding of host image buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    /// Scene-referred or display-referred linear light.
    #[default]
    Linear,
    /// IEC 61966-2-1 sRGB transfer curve.
    Srgb,
    /// ITU-R BT.709 transfer curve.
    Rec709,
    /// ITU-R BT.2020 12-bit transfer curve.
    Rec2020,
}

/// Alpha premultiplication state reported by the host (`kOfxImagePropPreMultiplication`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreMultiplication {
    /// Image has no meaningful transparency; alpha is always `1.0`.
    Opaque,
    /// RGB channels are premultiplied by alpha (`r_pre = r * a`).
    #[default]
    PreMultiplied,
    /// RGB channels are straight / unassociated with alpha.
    UnPreMultiplied,
}

/// Linear-light 32-bit floating-point RGBA pixel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rgba32F {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba32F {
    /// Fully transparent black pixel `(0, 0, 0, 0)`.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Opaque black pixel `(0, 0, 0, 1)`.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque white pixel `(1, 1, 1, 1)`.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Constructs a new RGBA pixel.
    #[inline]
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Replaces any non-finite channel with `0.0` and clamps alpha to `0.0..=1.0`.
    #[inline]
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            r: sanitize_f32(self.r, 0.0),
            g: sanitize_f32(self.g, 0.0),
            b: sanitize_f32(self.b, 0.0),
            a: safe_clamp_f32(self.a, 0.0, 1.0),
        }
    }

    /// Rec.709 linear luminance `Y = 0.2126 R + 0.7152 G + 0.0722 B`.
    #[inline]
    #[must_use]
    pub fn luma_rec709(self) -> f32 {
        let p = self.sanitized();
        0.2126 * p.r + 0.7152 * p.g + 0.0722 * p.b
    }

    /// Linearly interpolates between `self` and `other` with clamped factor `t`.
    #[inline]
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let f = safe_clamp_f32(t, 0.0, 1.0);
        let inv = 1.0 - f;
        Self {
            r: self.r * inv + other.r * f,
            g: self.g * inv + other.g * f,
            b: self.b * inv + other.b * f,
            a: self.a * inv + other.a * f,
        }
    }

    /// Scales RGB channels uniformly while preserving alpha.
    #[inline]
    #[must_use]
    pub fn scale_rgb(self, factor: f32) -> Self {
        let f = sanitize_f32(factor, 0.0);
        Self {
            r: self.r * f,
            g: self.g * f,
            b: self.b * f,
            a: self.a,
        }
    }

    /// Converts from encoded `color_space` to linear light (alpha remains linear).
    #[inline]
    #[must_use]
    pub fn to_linear(self, color_space: ColorSpace) -> Self {
        let p = self.sanitized();
        match color_space {
            ColorSpace::Linear => p,
            ColorSpace::Srgb => Self {
                r: srgb_to_linear_channel(p.r),
                g: srgb_to_linear_channel(p.g),
                b: srgb_to_linear_channel(p.b),
                a: p.a,
            },
            ColorSpace::Rec709 => Self {
                r: rec709_to_linear_channel(p.r),
                g: rec709_to_linear_channel(p.g),
                b: rec709_to_linear_channel(p.b),
                a: p.a,
            },
            ColorSpace::Rec2020 => Self {
                r: rec2020_to_linear_channel(p.r),
                g: rec2020_to_linear_channel(p.g),
                b: rec2020_to_linear_channel(p.b),
                a: p.a,
            },
        }
    }

    /// Converts from linear light to encoded `color_space` (alpha remains linear).
    #[inline]
    #[must_use]
    pub fn from_linear(self, color_space: ColorSpace) -> Self {
        let p = self.sanitized();
        match color_space {
            ColorSpace::Linear => p,
            ColorSpace::Srgb => Self {
                r: linear_to_srgb_channel(p.r),
                g: linear_to_srgb_channel(p.g),
                b: linear_to_srgb_channel(p.b),
                a: p.a,
            },
            ColorSpace::Rec709 => Self {
                r: linear_to_rec709_channel(p.r),
                g: linear_to_rec709_channel(p.g),
                b: linear_to_rec709_channel(p.b),
                a: p.a,
            },
            ColorSpace::Rec2020 => Self {
                r: linear_to_rec2020_channel(p.r),
                g: linear_to_rec2020_channel(p.g),
                b: linear_to_rec2020_channel(p.b),
                a: p.a,
            },
        }
    }

    /// Converts a pixel into straight (unassociated) RGBA safely without RGB leakage
    /// on transparent pixels.
    #[inline]
    #[must_use]
    pub fn to_straight(self, premult: PreMultiplication) -> Self {
        let p = self.sanitized();
        match premult {
            PreMultiplication::Opaque => Self {
                r: p.r,
                g: p.g,
                b: p.b,
                a: 1.0,
            },
            PreMultiplication::UnPreMultiplied => {
                if p.a <= 1e-6 {
                    Self::TRANSPARENT
                } else {
                    p
                }
            }
            PreMultiplication::PreMultiplied => {
                if p.a <= 1e-6 {
                    Self::TRANSPARENT
                } else {
                    let inv_a = 1.0 / p.a;
                    Self {
                        r: p.r * inv_a,
                        g: p.g * inv_a,
                        b: p.b * inv_a,
                        a: p.a,
                    }
                }
            }
        }
    }

    /// Converts a straight (unassociated) RGBA pixel into the host premultiplication state.
    #[inline]
    #[must_use]
    pub fn from_straight(self, premult: PreMultiplication) -> Self {
        let p = self.sanitized();
        match premult {
            PreMultiplication::Opaque => Self {
                r: p.r,
                g: p.g,
                b: p.b,
                a: 1.0,
            },
            PreMultiplication::UnPreMultiplied => {
                if p.a <= 1e-6 {
                    Self::TRANSPARENT
                } else {
                    p
                }
            }
            PreMultiplication::PreMultiplied => {
                if p.a <= 1e-6 {
                    Self::TRANSPARENT
                } else {
                    Self {
                        r: p.r * p.a,
                        g: p.g * p.a,
                        b: p.b * p.a,
                        a: p.a,
                    }
                }
            }
        }
    }

    /// Converts a straight pixel to premultiplied linear form (optimal for blur/filtering).
    #[inline]
    #[must_use]
    pub fn premultiply(self) -> Self {
        let p = self.sanitized();
        if p.a <= 1e-6 {
            Self::TRANSPARENT
        } else {
            Self {
                r: p.r * p.a,
                g: p.g * p.a,
                b: p.b * p.a,
                a: p.a,
            }
        }
    }

    /// Converts a premultiplied pixel back to straight form cleanly.
    #[inline]
    #[must_use]
    pub fn unpremultiply(self) -> Self {
        self.to_straight(PreMultiplication::PreMultiplied)
    }

    /// Porter-Duff `self OVER background` in straight linear RGBA.
    #[inline]
    #[must_use]
    pub fn over(self, background: Self) -> Self {
        let fg = self.sanitized();
        let bg = background.sanitized();
        let out_a = fg.a + bg.a * (1.0 - fg.a);
        if out_a <= 1e-6 {
            return Self::TRANSPARENT;
        }
        let inv_a = 1.0 / out_a;
        Self {
            r: (fg.r * fg.a + bg.r * bg.a * (1.0 - fg.a)) * inv_a,
            g: (fg.g * fg.a + bg.g * bg.a * (1.0 - fg.a)) * inv_a,
            b: (fg.b * fg.a + bg.b * bg.a * (1.0 - fg.a)) * inv_a,
            a: safe_clamp_f32(out_a, 0.0, 1.0),
        }
    }
}

#[inline]
fn srgb_to_linear_channel(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb_channel(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
fn rec709_to_linear_channel(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    if v < 0.081 {
        v / 4.5
    } else {
        ((v + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

#[inline]
fn linear_to_rec709_channel(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    if v < 0.018 {
        v * 4.5
    } else {
        1.099 * v.powf(0.45) - 0.099
    }
}

#[inline]
fn rec2020_to_linear_channel(v: f32) -> f32 {
    const ALPHA: f32 = 1.099_296_8;
    const BETA: f32 = 0.018_053_968;
    if v <= 0.0 {
        return 0.0;
    }
    if v < 4.5 * BETA {
        v / 4.5
    } else {
        ((v + (ALPHA - 1.0)) / ALPHA).powf(1.0 / 0.45)
    }
}

#[inline]
fn linear_to_rec2020_channel(v: f32) -> f32 {
    const ALPHA: f32 = 1.099_296_8;
    const BETA: f32 = 0.018_053_968;
    if v <= 0.0 {
        return 0.0;
    }
    if v < BETA {
        v * 4.5
    } else {
        ALPHA * v.powf(0.45) - (ALPHA - 1.0)
    }
}
