/// Maximum permitted width or height for a single rendered frame (16K ceiling).
pub const MAX_FRAME_DIMENSION: u32 = 16_384;

/// Maximum pixel count permitted in a single temporary linear frame allocation
/// (8192 x 8192 = 67,108,864 pixels).
pub const MAX_ALLOCATION_PIXELS: usize = 8_192 * 8_192;

/// Integer rectangle using half-open OpenFX bounds `[x1, x2) x [y1, y2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RectI {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl RectI {
    /// Creates an integer rectangle.
    #[must_use]
    pub const fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Returns non-negative width using saturating arithmetic.
    #[must_use]
    pub const fn width(self) -> u32 {
        let diff = self.x2.saturating_sub(self.x1);
        if diff <= 0 { 0 } else { diff as u32 }
    }

    /// Returns non-negative height using saturating arithmetic.
    #[must_use]
    pub const fn height(self) -> u32 {
        let diff = self.y2.saturating_sub(self.y1);
        if diff <= 0 { 0 } else { diff as u32 }
    }

    /// Returns `true` if the rectangle has zero area.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.x2 <= self.x1 || self.y2 <= self.y1
    }

    /// Converts integer bounds to floating-point canonical bounds.
    #[must_use]
    pub const fn to_rect_d(self) -> RectD {
        RectD {
            x1: self.x1 as f64,
            y1: self.y1 as f64,
            x2: self.x2 as f64,
            y2: self.y2 as f64,
        }
    }
}

/// Floating-point canonical rectangle used by OpenFX RoD / RoI actions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RectD {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl RectD {
    /// Creates a sanitized floating-point rectangle.
    #[must_use]
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x1: sanitize_f64(x1, 0.0),
            y1: sanitize_f64(y1, 0.0),
            x2: sanitize_f64(x2, 0.0),
            y2: sanitize_f64(y2, 0.0),
        }
    }

    /// Returns non-negative width.
    #[must_use]
    pub fn width(self) -> f64 {
        let w = sanitize_f64(self.x2 - self.x1, 0.0);
        if w > 0.0 { w } else { 0.0 }
    }

    /// Returns non-negative height.
    #[must_use]
    pub fn height(self) -> f64 {
        let h = sanitize_f64(self.y2 - self.y1, 0.0);
        if h > 0.0 { h } else { 0.0 }
    }

    /// Expands the rectangle symmetrically by non-negative finite margins.
    #[must_use]
    pub fn expand(self, pad_x: f64, pad_y: f64) -> Self {
        let px = safe_clamp_f64(pad_x, 0.0, f64::from(MAX_FRAME_DIMENSION));
        let py = safe_clamp_f64(pad_y, 0.0, f64::from(MAX_FRAME_DIMENSION));
        Self::new(self.x1 - px, self.y1 - py, self.x2 + px, self.y2 + py)
    }

    /// Returns the union of two rectangles, ignoring empty inputs.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        if self.width() <= 0.0 || self.height() <= 0.0 {
            return other;
        }
        if other.width() <= 0.0 || other.height() <= 0.0 {
            return self;
        }
        Self::new(
            self.x1.min(other.x1),
            self.y1.min(other.y1),
            self.x2.max(other.x2),
            self.y2.max(other.y2),
        )
    }
}

/// Replaces `NaN` and `±inf` with `fallback`.
#[inline]
#[must_use]
pub fn sanitize_f32(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else if fallback.is_finite() {
        fallback
    } else {
        0.0
    }
}

/// Replaces `NaN` and `±inf` with `fallback`.
#[inline]
#[must_use]
pub fn sanitize_f64(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else if fallback.is_finite() {
        fallback
    } else {
        0.0
    }
}

/// Panic-free clamp that tolerates inverted bounds (`lo > hi`) and non-finite inputs.
#[inline]
#[must_use]
pub fn safe_clamp_f32(value: f32, lo: f32, hi: f32) -> f32 {
    let a = sanitize_f32(lo, 0.0);
    let b = sanitize_f32(hi, a);
    let (min_v, max_v) = if a <= b { (a, b) } else { (b, a) };
    let v = sanitize_f32(value, min_v);
    v.max(min_v).min(max_v)
}

/// Panic-free clamp that tolerates inverted bounds (`lo > hi`) and non-finite inputs.
#[inline]
#[must_use]
pub fn safe_clamp_f64(value: f64, lo: f64, hi: f64) -> f64 {
    let a = sanitize_f64(lo, 0.0);
    let b = sanitize_f64(hi, a);
    let (min_v, max_v) = if a <= b { (a, b) } else { (b, a) };
    let v = sanitize_f64(value, min_v);
    v.max(min_v).min(max_v)
}

/// Safe division guarding zero, near-zero, and non-finite operands.
#[inline]
#[must_use]
pub fn safe_div_f32(num: f32, den: f32, fallback: f32) -> f32 {
    if !num.is_finite() || !den.is_finite() || den.abs() < 1e-12 {
        return sanitize_f32(fallback, 0.0);
    }
    sanitize_f32(num / den, fallback)
}

/// Safe division guarding zero, near-zero, and non-finite operands.
#[inline]
#[must_use]
pub fn safe_div_f64(num: f64, den: f64, fallback: f64) -> f64 {
    if !num.is_finite() || !den.is_finite() || den.abs() < 1e-15 {
        return sanitize_f64(fallback, 0.0);
    }
    sanitize_f64(num / den, fallback)
}

/// Converts a normalized `0.0..=1.0` float sample to `u8` safely.
#[inline]
#[must_use]
pub fn safe_f32_to_u8(value: f32) -> u8 {
    let clamped = safe_clamp_f32(value, 0.0, 1.0);
    (clamped * 255.0 + 0.5) as u8
}

/// Converts a normalized `0.0..=1.0` float sample to `u16` safely.
#[inline]
#[must_use]
pub fn safe_f32_to_u16(value: f32) -> u16 {
    let clamped = safe_clamp_f32(value, 0.0, 1.0);
    (clamped * 65_535.0 + 0.5) as u16
}

/// Converts an arbitrary float to a bounded `i32` without UB or panics.
#[inline]
#[must_use]
pub fn safe_f32_to_i32(value: f32, lo: i32, hi: i32) -> i32 {
    let (min_i, max_i) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let v = sanitize_f32(value, min_i as f32).round();
    let clamped = safe_clamp_f64(f64::from(v), f64::from(min_i), f64::from(max_i));
    clamped as i32
}

/// Converts a resolution-independent normalized radius (fraction of reference frame
/// extent, e.g. `0.02` = 2% of min/reference dimension) into a capped pixel radius.
#[must_use]
pub fn normalized_radius_to_pixels(
    normalized_radius: f64,
    width: u32,
    height: u32,
    max_pixels: u32,
) -> u32 {
    let norm = safe_clamp_f64(normalized_radius, 0.0, 1.0);
    let ref_dim = f64::from(width.min(height));
    let px = (norm * ref_dim).round();
    let clamped = safe_clamp_f64(px, 0.0, f64::from(max_pixels));
    clamped as u32
}
