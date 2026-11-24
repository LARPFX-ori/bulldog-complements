use crate::abort::AbortCheck;
use crate::color::Rgba32F;
use crate::error::{CoreError, CoreResult};
use crate::image::LinearFrame;
use crate::render::render_rows_parallel;

/// Maximum pixel blur radius permitted by the separable box blur kernel to bound
/// processing latency on extreme parameter inputs (§0.5).
pub const MAX_BLUR_RADIUS_PX: u32 = 256;

/// Applies a 3-pass separable sliding-window box blur (closely approximating a Gaussian
/// kernel) in premultiplied linear space, preserving straight alpha cleanly.
///
/// Runs in `O(W * H)` independent of radius, uses zero per-pixel heap allocations,
/// executes rows/columns in parallel with `rayon`, and checks `abort` cooperatively.
pub fn gaussian_approx_blur(
    src: &LinearFrame,
    dst: &mut LinearFrame,
    scratch: &mut LinearFrame,
    radius_px: u32,
    abort: &dyn AbortCheck,
) -> CoreResult<()> {
    let width = src.width();
    let height = src.height();
    let r = radius_px.min(MAX_BLUR_RADIUS_PX);

    if r == 0 {
        return dst.copy_from(src);
    }

    dst.resize_clear(width, height)?;
    scratch.resize_clear(width, height)?;

    // Convert input to premultiplied linear into `dst` so transparent neighbors do not
    // bleed undefined RGB values during filtering (§3.2).
    let src_pixels = src.pixels();
    render_rows_parallel(dst, abort, |y, dst_row| {
        let row_start = (y as usize).saturating_mul(width as usize);
        if let Some(s_row) = src_pixels.get(row_start..row_start + width as usize) {
            for (s, d) in s_row.iter().zip(dst_row.iter_mut()) {
                *d = s.premultiply();
            }
        }
    })?;

    // Three box-blur passes approximate a smooth Gaussian impulse response.
    for _pass in 0..3 {
        if abort.is_aborted() {
            return Err(CoreError::Aborted);
        }
        box_blur_horizontal(dst, scratch, r, abort)?;
        box_blur_vertical(scratch, dst, r, abort)?;
    }

    // Unpremultiply back into straight linear space.
    render_rows_parallel(dst, abort, |_y, row| {
        for px in row.iter_mut() {
            *px = px.unpremultiply();
        }
    })?;

    Ok(())
}

fn box_blur_horizontal(
    src: &LinearFrame,
    dst: &mut LinearFrame,
    radius: u32,
    abort: &dyn AbortCheck,
) -> CoreResult<()> {
    let width = src.width() as usize;
    if width == 0 {
        return Ok(());
    }
    let r = radius as usize;
    let kernel_len = r.saturating_mul(2).saturating_add(1);
    let inv_k = 1.0 / (kernel_len as f32);
    let max_x = width.saturating_sub(1);

    render_rows_parallel(dst, abort, |y, dst_row| {
        let Some(src_row) = src.row(y) else {
            return;
        };
        let Some(&first_px) = src_row.first() else {
            return;
        };
        let Some(&last_px) = src_row.last() else {
            return;
        };

        // Initialize sliding sum at x = 0: `(r + 1)` copies of left edge + pixels 1..=r.
        let mut acc_r = first_px.r * (r + 1) as f32;
        let mut acc_g = first_px.g * (r + 1) as f32;
        let mut acc_b = first_px.b * (r + 1) as f32;
        let mut acc_a = first_px.a * (r + 1) as f32;

        for offset in 1..=r {
            let sample = src_row.get(offset.min(max_x)).copied().unwrap_or(last_px);
            acc_r += sample.r;
            acc_g += sample.g;
            acc_b += sample.b;
            acc_a += sample.a;
        }

        for (x, out_px) in dst_row.iter_mut().enumerate() {
            *out_px = Rgba32F::new(acc_r * inv_k, acc_g * inv_k, acc_b * inv_k, acc_a * inv_k);

            let out_idx = x.saturating_sub(r);
            let in_idx = x.saturating_add(r + 1).min(max_x);
            let leaving = if x < r {
                first_px
            } else {
                src_row.get(out_idx).copied().unwrap_or(first_px)
            };
            let entering = src_row.get(in_idx).copied().unwrap_or(last_px);

            acc_r += entering.r - leaving.r;
            acc_g += entering.g - leaving.g;
            acc_b += entering.b - leaving.b;
            acc_a += entering.a - leaving.a;
        }
    })
}

fn box_blur_vertical(
    src: &LinearFrame,
    dst: &mut LinearFrame,
    radius: u32,
    abort: &dyn AbortCheck,
) -> CoreResult<()> {
    let width = src.width() as usize;
    let height = src.height() as usize;
    if width == 0 || height == 0 {
        return Ok(());
    }
    let r = radius as usize;
    let kernel_len = r.saturating_mul(2).saturating_add(1);
    let inv_k = 1.0 / (kernel_len as f32);
    let max_y = height.saturating_sub(1);
    let src_pixels = src.pixels();

    // Row-parallel vertical box filter: each output row `y` sums `[y - r ..= y + r]`
    // cleanly without sharing mutable column state across threads.
    render_rows_parallel(dst, abort, |y, dst_row| {
        let yi = y as isize;
        let ri = r as isize;

        for px in dst_row.iter_mut() {
            *px = Rgba32F::TRANSPARENT;
        }

        for dy in -ri..=ri {
            let sy = (yi + dy).clamp(0, max_y as isize) as usize;
            let start = sy.saturating_mul(width);
            if let Some(s_row) = src_pixels.get(start..start + width) {
                for (acc, s) in dst_row.iter_mut().zip(s_row.iter()) {
                    acc.r += s.r;
                    acc.g += s.g;
                    acc.b += s.b;
                    acc.a += s.a;
                }
            }
        }

        for acc in dst_row.iter_mut() {
            acc.r *= inv_k;
            acc.g *= inv_k;
            acc.b *= inv_k;
            acc.a *= inv_k;
        }
    })
}
