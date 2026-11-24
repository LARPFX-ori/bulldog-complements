use crate::math::sanitize_f64;

/// Converts a timeline frame/time coordinate into a deterministic integer key.
///
/// Quantizes to micro-frames so random-access renders at identical times always
/// produce identical keys, and non-finite inputs map deterministically to `0`.
#[inline]
#[must_use]
pub fn frame_time_to_key(frame_time: f64) -> u64 {
    let t = sanitize_f64(frame_time, 0.0);
    let scaled = (t * 1_000_000.0).round();
    let clamped = scaled.clamp(i64::MIN as f64 / 2.0, i64::MAX as f64 / 2.0);
    (clamped as i64) as u64
}

/// SplitMix64 mixing function for fast, stateless, high-quality deterministic hashes.
#[inline]
#[must_use]
pub const fn hash_u64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Deterministic spatial+temporal hash returning a uniform `f32` in `[0.0, 1.0)`.
#[inline]
#[must_use]
pub fn hash_2d(x: i32, y: i32, frame_key: u64, seed: u64) -> f32 {
    let coord = ((x as u32 as u64) << 32) | (y as u32 as u64);
    let mixed = hash_u64(
        coord
            ^ hash_u64(frame_key.wrapping_mul(0x517c_c1b7_2722_0a95))
            ^ hash_u64(seed.wrapping_add(0x2545_f491_4f6c_dd1d)),
    );
    // 24 bits of mantissa precision -> [0.0, 1.0)
    ((mixed >> 40) as f32) * (1.0 / 16_777_216.0)
}

/// Deterministic spatial+temporal hash returning a uniform `f32` in `[-1.0, 1.0)`.
#[inline]
#[must_use]
pub fn hash_2d_signed(x: i32, y: i32, frame_key: u64, seed: u64) -> f32 {
    hash_2d(x, y, frame_key, seed) * 2.0 - 1.0
}

/// Deterministic SplitMix64 PRNG seeded strictly from `(frame_key, seed, salt)`.
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    /// Creates a deterministic PRNG independent of previous render history.
    #[must_use]
    pub const fn new(frame_key: u64, seed: u64, salt: u64) -> Self {
        let s0 = hash_u64(frame_key ^ hash_u64(seed) ^ hash_u64(salt.wrapping_mul(0x9e37_79b1)));
        Self { state: s0 }
    }

    /// Generates the next 64-bit unsigned integer.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        hash_u64(self.state)
    }

    /// Generates a uniform `f32` in `[0.0, 1.0)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) * (1.0 / 16_777_216.0)
    }

    /// Generates a uniform `f32` in `[lo, hi)`.
    #[inline]
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        let (a, b) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        a + (b - a) * self.next_f32()
    }
}
