use crate::abort::AbortCheck;
use crate::color::{ColorSpace, PreMultiplication, Rgba32F};
use crate::error::{CoreError, CoreResult};
use crate::math::{
    MAX_ALLOCATION_PIXELS, MAX_FRAME_DIMENSION, safe_clamp_f32, safe_f32_to_u8, safe_f32_to_u16,
    sanitize_f32,
};

/// Maximum number of scratch frames retained in a single [`BufferPool`].
pub const MAX_POOL_BUFFERS: usize = 8;

/// Supported host pixel storage formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit unsigned integer RGBA (4 bytes per pixel).
    Rgba8,
    /// 16-bit unsigned integer RGBA in native endianness (8 bytes per pixel).
    Rgba16,
    /// 32-bit IEEE-754 float RGBA in native endianness (16 bytes per pixel).
    Rgba32F,
}

impl PixelFormat {
    /// Returns the number of bytes occupied by one pixel.
    #[inline]
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8 => 4,
            Self::Rgba16 => 8,
            Self::Rgba32F => 16,
        }
    }
}

/// Validates host image layout (including padded and negative `row_bytes` strides)
/// and returns `(min_row_bytes, abs_stride)`.
fn validate_host_layout(
    data_len: usize,
    width: u32,
    height: u32,
    format: PixelFormat,
    row_bytes: isize,
    base_offset: usize,
) -> CoreResult<(usize, usize)> {
    if width == 0 || height == 0 || width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION {
        return Err(CoreError::InvalidDimensions { width, height });
    }
    let bpp = format.bytes_per_pixel();
    let min_row_bytes = (width as usize)
        .checked_mul(bpp)
        .ok_or(CoreError::InvalidDimensions { width, height })?;

    let abs_stride = row_bytes.unsigned_abs();
    if abs_stride < min_row_bytes {
        return Err(CoreError::InvalidStride {
            row_bytes,
            min_row_bytes,
        });
    }

    let last_row = (height as usize).saturating_sub(1);
    let span = last_row
        .checked_mul(abs_stride)
        .ok_or(CoreError::InvalidStride {
            row_bytes,
            min_row_bytes,
        })?;

    if row_bytes >= 0 {
        let last_start = base_offset
            .checked_add(span)
            .ok_or(CoreError::InvalidStride {
                row_bytes,
                min_row_bytes,
            })?;
        let end = last_start
            .checked_add(min_row_bytes)
            .ok_or(CoreError::InvalidStride {
                row_bytes,
                min_row_bytes,
            })?;
        if end > data_len {
            return Err(CoreError::BufferTooSmall {
                required: end,
                actual: data_len,
            });
        }
    } else {
        // Negative row_bytes: row 0 sits at `base_offset` and row `y` walks backward.
        let min_start = base_offset
            .checked_sub(span)
            .ok_or(CoreError::InvalidStride {
                row_bytes,
                min_row_bytes,
            })?;
        let _ = min_start;
        let max_end = base_offset
            .checked_add(min_row_bytes)
            .ok_or(CoreError::InvalidStride {
                row_bytes,
                min_row_bytes,
            })?;
        if max_end > data_len {
            return Err(CoreError::BufferTooSmall {
                required: max_end,
                actual: data_len,
            });
        }
    }

    Ok((min_row_bytes, abs_stride))
}

#[inline]
fn row_byte_range(
    y: u32,
    height: u32,
    row_bytes: isize,
    base_offset: usize,
    row_len: usize,
) -> Option<core::ops::Range<usize>> {
    if y >= height {
        return None;
    }
    let abs_stride = row_bytes.unsigned_abs();
    let delta = (y as usize).checked_mul(abs_stride)?;
    let start = if row_bytes >= 0 {
        base_offset.checked_add(delta)?
    } else {
        base_offset.checked_sub(delta)?
    };
    let end = start.checked_add(row_len)?;
    Some(start..end)
}

/// Safe immutable view over a host-supplied pixel buffer with arbitrary stride.
#[derive(Debug, Clone, Copy)]
pub struct HostImageRef<'a> {
    data: &'a [u8],
    width: u32,
    height: u32,
    format: PixelFormat,
    row_bytes: isize,
    base_offset: usize,
    row_len: usize,
}

impl<'a> HostImageRef<'a> {
    /// Creates a validated immutable host buffer view.
    ///
    /// For top-down / positive `row_bytes`, pass `base_offset = 0`.
    /// For bottom-up / negative `row_bytes`, pass `base_offset = (height - 1) * abs(row_bytes)`.
    pub fn new(
        data: &'a [u8],
        width: u32,
        height: u32,
        format: PixelFormat,
        row_bytes: isize,
        base_offset: usize,
    ) -> CoreResult<Self> {
        let (row_len, _abs_stride) =
            validate_host_layout(data.len(), width, height, format, row_bytes, base_offset)?;
        Ok(Self {
            data,
            width,
            height,
            format,
            row_bytes,
            base_offset,
            row_len,
        })
    }

    /// Convenience constructor for tightly packed top-down host buffers.
    pub fn tightly_packed(
        data: &'a [u8],
        width: u32,
        height: u32,
        format: PixelFormat,
    ) -> CoreResult<Self> {
        let stride_usize = (width as usize)
            .checked_mul(format.bytes_per_pixel())
            .ok_or(CoreError::InvalidDimensions { width, height })?;
        let stride = isize::try_from(stride_usize).map_err(|_| CoreError::InvalidStride {
            row_bytes: isize::MAX,
            min_row_bytes: stride_usize,
        })?;
        Self::new(data, width, height, format, stride, 0)
    }

    #[inline]
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    #[must_use]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }

    #[inline]
    #[must_use]
    pub const fn row_bytes(&self) -> isize {
        self.row_bytes
    }

    /// Returns the exact byte slice for row `y` respecting stride sign and padding.
    #[inline]
    #[must_use]
    pub fn row_slice(&self, y: u32) -> Option<&'a [u8]> {
        let range = row_byte_range(
            y,
            self.height,
            self.row_bytes,
            self.base_offset,
            self.row_len,
        )?;
        self.data.get(range)
    }
}

/// Safe mutable view over a host-supplied output pixel buffer with arbitrary stride.
#[derive(Debug)]
pub struct HostImageMut<'a> {
    data: &'a mut [u8],
    width: u32,
    height: u32,
    format: PixelFormat,
    row_bytes: isize,
    base_offset: usize,
    row_len: usize,
}

impl<'a> HostImageMut<'a> {
    /// Creates a validated mutable host buffer view.
    pub fn new(
        data: &'a mut [u8],
        width: u32,
        height: u32,
        format: PixelFormat,
        row_bytes: isize,
        base_offset: usize,
    ) -> CoreResult<Self> {
        let (row_len, _abs_stride) =
            validate_host_layout(data.len(), width, height, format, row_bytes, base_offset)?;
        Ok(Self {
            data,
            width,
            height,
            format,
            row_bytes,
            base_offset,
            row_len,
        })
    }

    /// Convenience constructor for tightly packed top-down mutable buffers.
    pub fn tightly_packed(
        data: &'a mut [u8],
        width: u32,
        height: u32,
        format: PixelFormat,
    ) -> CoreResult<Self> {
        let stride_usize = (width as usize)
            .checked_mul(format.bytes_per_pixel())
            .ok_or(CoreError::InvalidDimensions { width, height })?;
        let stride = isize::try_from(stride_usize).map_err(|_| CoreError::InvalidStride {
            row_bytes: isize::MAX,
            min_row_bytes: stride_usize,
        })?;
        Self::new(data, width, height, format, stride, 0)
    }

    #[inline]
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    #[must_use]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }

    #[inline]
    #[must_use]
    pub const fn row_bytes(&self) -> isize {
        self.row_bytes
    }

    /// Returns the exact mutable byte slice for row `y`.
    #[inline]
    #[must_use]
    pub fn row_slice_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        let range = row_byte_range(
            y,
            self.height,
            self.row_bytes,
            self.base_offset,
            self.row_len,
        )?;
        self.data.get_mut(range)
    }
}

/// Internal contiguous linear-light `f32` RGBA frame buffer.
///
/// Pixels are stored in **straight** linear RGBA (`Rgba32F`) so all optical effects
/// avoid dark fringes and preserve HDR dynamic range.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearFrame {
    width: u32,
    height: u32,
    pixels: Vec<Rgba32F>,
}

impl LinearFrame {
    /// Allocates a bounded linear frame filled with transparent black.
    pub fn try_new(width: u32, height: u32) -> CoreResult<Self> {
        let pixel_count = validate_linear_dims(width, height)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(pixel_count)
            .map_err(|_| CoreError::AllocationLimitExceeded {
                requested_pixels: pixel_count,
                max_pixels: MAX_ALLOCATION_PIXELS,
            })?;
        pixels.resize(pixel_count, Rgba32F::TRANSPARENT);
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Reuses this frame's backing memory for `(width, height)`, clearing pixels to transparent.
    pub fn resize_clear(&mut self, width: u32, height: u32) -> CoreResult<()> {
        let pixel_count = validate_linear_dims(width, height)?;
        if pixel_count > self.pixels.capacity() {
            let additional = pixel_count.saturating_sub(self.pixels.len());
            self.pixels.try_reserve_exact(additional).map_err(|_| {
                CoreError::AllocationLimitExceeded {
                    requested_pixels: pixel_count,
                    max_pixels: MAX_ALLOCATION_PIXELS,
                }
            })?;
        }
        self.pixels.clear();
        self.pixels.resize(pixel_count, Rgba32F::TRANSPARENT);
        self.width = width;
        self.height = height;
        Ok(())
    }

    #[inline]
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    #[must_use]
    pub fn pixels(&self) -> &[Rgba32F] {
        &self.pixels
    }

    #[inline]
    #[must_use]
    pub fn pixels_mut(&mut self) -> &mut [Rgba32F] {
        &mut self.pixels
    }

    /// Reads pixel `(x, y)` safely, returning `None` if out of bounds.
    #[inline]
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba32F> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?;
        self.pixels.get(idx).copied()
    }

    /// Reads pixel `(x, y)` clamping coordinates to frame edges.
    #[inline]
    #[must_use]
    pub fn get_pixel_clamped(&self, x: i32, y: i32) -> Rgba32F {
        if self.width == 0 || self.height == 0 {
            return Rgba32F::TRANSPARENT;
        }
        let cx = x.clamp(0, (self.width - 1) as i32) as u32;
        let cy = y.clamp(0, (self.height - 1) as i32) as u32;
        self.get_pixel(cx, cy).unwrap_or(Rgba32F::TRANSPARENT)
    }

    /// Sets pixel `(x, y)` if inside bounds.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: Rgba32F) {
        if x >= self.width || y >= self.height {
            return;
        }
        if let Some(idx) = (y as usize)
            .checked_mul(self.width as usize)
            .and_then(|row| row.checked_add(x as usize))
            && let Some(slot) = self.pixels.get_mut(idx)
        {
            *slot = pixel;
        }
    }

    /// Returns row `y` as a slice.
    #[inline]
    #[must_use]
    pub fn row(&self, y: u32) -> Option<&[Rgba32F]> {
        if y >= self.height {
            return None;
        }
        let start = (y as usize).checked_mul(self.width as usize)?;
        let end = start.checked_add(self.width as usize)?;
        self.pixels.get(start..end)
    }

    /// Returns row `y` as a mutable slice.
    #[inline]
    #[must_use]
    pub fn row_mut(&mut self, y: u32) -> Option<&mut [Rgba32F]> {
        if y >= self.height {
            return None;
        }
        let start = (y as usize).checked_mul(self.width as usize)?;
        let end = start.checked_add(self.width as usize)?;
        self.pixels.get_mut(start..end)
    }

    /// Copies pixels from another frame of matching dimensions (resizing if needed).
    pub fn copy_from(&mut self, src: &Self) -> CoreResult<()> {
        if self.width != src.width || self.height != src.height {
            self.resize_clear(src.width, src.height)?;
        }
        if self.pixels.len() == src.pixels.len() {
            self.pixels.copy_from_slice(&src.pixels);
        }
        Ok(())
    }

    /// Bilinearly samples the frame in normalized UV coordinates `u, v in [0, 1]`.
    ///
    /// When `clamp_edge` is `false`, samples outside `[0, 1]` return transparent black.
    /// Premultiplies during interpolation to prevent dark edge halos around transparent cutouts.
    #[must_use]
    pub fn sample_bilinear(&self, u: f32, v: f32, clamp_edge: bool) -> Rgba32F {
        if self.width == 0 || self.height == 0 || !u.is_finite() || !v.is_finite() {
            return Rgba32F::TRANSPARENT;
        }
        if !clamp_edge && (!(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v)) {
            return Rgba32F::TRANSPARENT;
        }

        let cu = safe_clamp_f32(u, 0.0, 1.0);
        let cv = safe_clamp_f32(v, 0.0, 1.0);
        let max_x = (self.width - 1) as f32;
        let max_y = (self.height - 1) as f32;

        let fx = cu * max_x;
        let fy = cv * max_y;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let x1 = x0.saturating_add(1);
        let y1 = y0.saturating_add(1);
        let tx = safe_clamp_f32(fx - x0 as f32, 0.0, 1.0);
        let ty = safe_clamp_f32(fy - y0 as f32, 0.0, 1.0);

        let p00 = self.get_pixel_clamped(x0, y0).premultiply();
        let p10 = self.get_pixel_clamped(x1, y0).premultiply();
        let p01 = self.get_pixel_clamped(x0, y1).premultiply();
        let p11 = self.get_pixel_clamped(x1, y1).premultiply();

        let top = p00.lerp(p10, tx);
        let bot = p01.lerp(p11, tx);
        top.lerp(bot, ty).unpremultiply()
    }
}

fn validate_linear_dims(width: u32, height: u32) -> CoreResult<usize> {
    if width == 0 || height == 0 || width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION {
        return Err(CoreError::InvalidDimensions { width, height });
    }
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or(CoreError::InvalidDimensions { width, height })?;
    if pixel_count > MAX_ALLOCATION_PIXELS {
        return Err(CoreError::AllocationLimitExceeded {
            requested_pixels: pixel_count,
            max_pixels: MAX_ALLOCATION_PIXELS,
        });
    }
    Ok(pixel_count)
}

/// Per-instance reusable scratch frame pool preventing per-render heap churn (§3.5).
#[derive(Debug, Default)]
pub struct BufferPool {
    frames: Vec<LinearFrame>,
}

impl BufferPool {
    /// Creates an empty buffer pool.
    #[must_use]
    pub const fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// Acquires a cleared linear frame of size `(width, height)` reusing pooled capacity.
    pub fn acquire(&mut self, width: u32, height: u32) -> CoreResult<LinearFrame> {
        if let Some(mut frame) = self.frames.pop() {
            frame.resize_clear(width, height)?;
            return Ok(frame);
        }
        LinearFrame::try_new(width, height)
    }

    /// Returns a scratch frame to the pool for reuse (bounded by [`MAX_POOL_BUFFERS`]).
    pub fn release(&mut self, frame: LinearFrame) {
        if self.frames.len() < MAX_POOL_BUFFERS {
            self.frames.push(frame);
        }
    }

    /// Number of currently cached reusable buffers.
    #[must_use]
    pub fn cached_count(&self) -> usize {
        self.frames.len()
    }
}

/// Decodes a host buffer into a straight linear-light [`LinearFrame`].
pub fn decode_host_to_linear(
    src: &HostImageRef<'_>,
    dst: &mut LinearFrame,
    color_space: ColorSpace,
    premult: PreMultiplication,
    abort: &dyn AbortCheck,
) -> CoreResult<()> {
    dst.resize_clear(src.width(), src.height())?;
    let height = src.height();
    let format = src.format();

    for y in 0..height {
        if (y & 31) == 0 && abort.is_aborted() {
            return Err(CoreError::Aborted);
        }
        let Some(src_row) = src.row_slice(y) else {
            return Err(CoreError::Internal("source row out of bounds".to_owned()));
        };
        let Some(dst_row) = dst.row_mut(y) else {
            return Err(CoreError::Internal(
                "destination row out of bounds".to_owned(),
            ));
        };
        decode_row(src_row, dst_row, format, color_space, premult);
    }

    Ok(())
}

/// Encodes a straight linear-light [`LinearFrame`] back into a host buffer.
pub fn encode_linear_to_host(
    src: &LinearFrame,
    dst: &mut HostImageMut<'_>,
    color_space: ColorSpace,
    premult: PreMultiplication,
    abort: &dyn AbortCheck,
) -> CoreResult<()> {
    if src.width() != dst.width() || src.height() != dst.height() {
        return Err(CoreError::InvalidDimensions {
            width: dst.width(),
            height: dst.height(),
        });
    }
    let height = src.height();
    let format = dst.format();

    for y in 0..height {
        if (y & 31) == 0 && abort.is_aborted() {
            return Err(CoreError::Aborted);
        }
        let Some(src_row) = src.row(y) else {
            return Err(CoreError::Internal(
                "linear source row out of bounds".to_owned(),
            ));
        };
        let Some(dst_row) = dst.row_slice_mut(y) else {
            return Err(CoreError::Internal(
                "host output row out of bounds".to_owned(),
            ));
        };
        encode_row(src_row, dst_row, format, color_space, premult);
    }

    Ok(())
}

/// Copies a host source buffer directly to a host destination buffer as a safe bypass,
/// converting pixel formats when needed while respecting differing strides.
pub fn copy_host_bypass(src: &HostImageRef<'_>, dst: &mut HostImageMut<'_>) -> CoreResult<()> {
    let copy_h = src.height().min(dst.height());
    let copy_w = src.width().min(dst.width());

    if src.format() == dst.format() {
        let copy_bytes = (copy_w as usize)
            .checked_mul(src.format().bytes_per_pixel())
            .ok_or(CoreError::InvalidDimensions {
                width: copy_w,
                height: copy_h,
            })?;
        for y in 0..copy_h {
            if let (Some(s_row), Some(d_row)) = (src.row_slice(y), dst.row_slice_mut(y))
                && let (Some(s_part), Some(d_part)) =
                    (s_row.get(..copy_bytes), d_row.get_mut(..copy_bytes))
            {
                d_part.copy_from_slice(s_part);
            }
        }
        return Ok(());
    }

    // Cross-format fallback: row-by-row copy without allocating a full frame.
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(copy_w as usize)
        .map_err(|_| CoreError::AllocationLimitExceeded {
            requested_pixels: copy_w as usize,
            max_pixels: MAX_ALLOCATION_PIXELS,
        })?;
    scratch.resize(copy_w as usize, Rgba32F::TRANSPARENT);

    let src_fmt = src.format();
    let dst_fmt = dst.format();
    for y in 0..copy_h {
        if let (Some(s_row), Some(d_row)) = (src.row_slice(y), dst.row_slice_mut(y)) {
            decode_row(
                s_row,
                &mut scratch,
                src_fmt,
                ColorSpace::Linear,
                PreMultiplication::UnPreMultiplied,
            );
            encode_row(
                &scratch,
                d_row,
                dst_fmt,
                ColorSpace::Linear,
                PreMultiplication::UnPreMultiplied,
            );
        }
    }
    Ok(())
}

fn decode_row(
    src_row: &[u8],
    dst_row: &mut [Rgba32F],
    format: PixelFormat,
    color_space: ColorSpace,
    premult: PreMultiplication,
) {
    match format {
        PixelFormat::Rgba8 => {
            const INV_255: f32 = 1.0 / 255.0;
            for (chunk, dst) in src_row.chunks_exact(4).zip(dst_row.iter_mut()) {
                let raw = Rgba32F::new(
                    f32::from(chunk.first().copied().unwrap_or(0)) * INV_255,
                    f32::from(chunk.get(1).copied().unwrap_or(0)) * INV_255,
                    f32::from(chunk.get(2).copied().unwrap_or(0)) * INV_255,
                    f32::from(chunk.get(3).copied().unwrap_or(0)) * INV_255,
                );
                *dst = raw.to_straight(premult).to_linear(color_space);
            }
        }
        PixelFormat::Rgba16 => {
            const INV_65535: f32 = 1.0 / 65_535.0;
            for (chunk, dst) in src_row.chunks_exact(8).zip(dst_row.iter_mut()) {
                let r = read_u16_ne(chunk, 0);
                let g = read_u16_ne(chunk, 2);
                let b = read_u16_ne(chunk, 4);
                let a = read_u16_ne(chunk, 6);
                let raw = Rgba32F::new(
                    f32::from(r) * INV_65535,
                    f32::from(g) * INV_65535,
                    f32::from(b) * INV_65535,
                    f32::from(a) * INV_65535,
                );
                *dst = raw.to_straight(premult).to_linear(color_space);
            }
        }
        PixelFormat::Rgba32F => {
            for (chunk, dst) in src_row.chunks_exact(16).zip(dst_row.iter_mut()) {
                let raw = Rgba32F::new(
                    sanitize_f32(read_f32_ne(chunk, 0), 0.0),
                    sanitize_f32(read_f32_ne(chunk, 4), 0.0),
                    sanitize_f32(read_f32_ne(chunk, 8), 0.0),
                    sanitize_f32(read_f32_ne(chunk, 12), 0.0),
                );
                *dst = raw.to_straight(premult).to_linear(color_space);
            }
        }
    }
}

fn encode_row(
    src_row: &[Rgba32F],
    dst_row: &mut [u8],
    format: PixelFormat,
    color_space: ColorSpace,
    premult: PreMultiplication,
) {
    match format {
        PixelFormat::Rgba8 => {
            for (src, chunk) in src_row.iter().zip(dst_row.chunks_exact_mut(4)) {
                let encoded = src.from_linear(color_space).from_straight(premult);
                if let Some(c) = chunk.first_mut() {
                    *c = safe_f32_to_u8(encoded.r);
                }
                if let Some(c) = chunk.get_mut(1) {
                    *c = safe_f32_to_u8(encoded.g);
                }
                if let Some(c) = chunk.get_mut(2) {
                    *c = safe_f32_to_u8(encoded.b);
                }
                if let Some(c) = chunk.get_mut(3) {
                    *c = safe_f32_to_u8(encoded.a);
                }
            }
        }
        PixelFormat::Rgba16 => {
            for (src, chunk) in src_row.iter().zip(dst_row.chunks_exact_mut(8)) {
                let encoded = src.from_linear(color_space).from_straight(premult);
                write_u16_ne(chunk, 0, safe_f32_to_u16(encoded.r));
                write_u16_ne(chunk, 2, safe_f32_to_u16(encoded.g));
                write_u16_ne(chunk, 4, safe_f32_to_u16(encoded.b));
                write_u16_ne(chunk, 6, safe_f32_to_u16(encoded.a));
            }
        }
        PixelFormat::Rgba32F => {
            for (src, chunk) in src_row.iter().zip(dst_row.chunks_exact_mut(16)) {
                let encoded = src
                    .from_linear(color_space)
                    .from_straight(premult)
                    .sanitized();
                write_f32_ne(chunk, 0, encoded.r);
                write_f32_ne(chunk, 4, encoded.g);
                write_f32_ne(chunk, 8, encoded.b);
                write_f32_ne(chunk, 12, encoded.a);
            }
        }
    }
}

#[inline]
fn read_u16_ne(slice: &[u8], offset: usize) -> u16 {
    let b0 = slice.get(offset).copied().unwrap_or(0);
    let b1 = slice.get(offset + 1).copied().unwrap_or(0);
    u16::from_ne_bytes([b0, b1])
}

#[inline]
fn write_u16_ne(slice: &mut [u8], offset: usize, val: u16) {
    let bytes = val.to_ne_bytes();
    if let Some(dst) = slice.get_mut(offset..offset + 2) {
        dst.copy_from_slice(&bytes);
    }
}

#[inline]
fn read_f32_ne(slice: &[u8], offset: usize) -> f32 {
    let b0 = slice.get(offset).copied().unwrap_or(0);
    let b1 = slice.get(offset + 1).copied().unwrap_or(0);
    let b2 = slice.get(offset + 2).copied().unwrap_or(0);
    let b3 = slice.get(offset + 3).copied().unwrap_or(0);
    f32::from_ne_bytes([b0, b1, b2, b3])
}

#[inline]
fn write_f32_ne(slice: &mut [u8], offset: usize, val: f32) {
    let bytes = val.to_ne_bytes();
    if let Some(dst) = slice.get_mut(offset..offset + 4) {
        dst.copy_from_slice(&bytes);
    }
}
