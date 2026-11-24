use bulldog_core::dsp::gaussian_approx_blur;
use bulldog_core::image::{copy_host_bypass, decode_host_to_linear, encode_linear_to_host};
use bulldog_core::{
    AtomicAbort, BufferPool, ColorSpace, CoreError, DeterministicRng, HostImageMut, HostImageRef,
    LinearFrame, NeverAbort, ParamDescriptor, ParamSet, ParamValue, PixelFormat, PreMultiplication,
    Rgba32F, frame_time_to_key, hash_2d, safe_clamp_f32, safe_clamp_f64, safe_div_f32,
};

#[test]
fn hostile_dimensions_rejected_without_panic() {
    let buf = [0u8; 64];
    assert!(matches!(
        HostImageRef::tightly_packed(&buf, 0, 0, PixelFormat::Rgba8),
        Err(CoreError::InvalidDimensions {
            width: 0,
            height: 0
        })
    ));
    assert!(matches!(
        HostImageRef::tightly_packed(&buf, 0, 16, PixelFormat::Rgba8),
        Err(CoreError::InvalidDimensions {
            width: 0,
            height: 16
        })
    ));
    assert!(matches!(
        LinearFrame::try_new(0, 0),
        Err(CoreError::InvalidDimensions {
            width: 0,
            height: 0
        })
    ));
    assert!(matches!(
        LinearFrame::try_new(32_768, 32_768),
        Err(CoreError::InvalidDimensions { .. })
    ));
    // 16384x16384 exceeds MAX_ALLOCATION_PIXELS (8192x8192) -> bounded allocation rejection.
    assert!(matches!(
        LinearFrame::try_new(16_384, 16_384),
        Err(CoreError::AllocationLimitExceeded { .. })
    ));
}

#[test]
fn minimal_1x1_and_non_contiguous_negative_strides_roundtrip() {
    // 1x1 frame roundtrip
    let src_1x1 = [255u8, 128, 64, 255];
    let mut dst_1x1 = [0u8; 4];
    let src_view = HostImageRef::tightly_packed(&src_1x1, 1, 1, PixelFormat::Rgba8).unwrap();
    let mut dst_view =
        HostImageMut::tightly_packed(&mut dst_1x1, 1, 1, PixelFormat::Rgba8).unwrap();
    let mut frame = LinearFrame::try_new(1, 1).unwrap();
    decode_host_to_linear(
        &src_view,
        &mut frame,
        ColorSpace::Srgb,
        PreMultiplication::UnPreMultiplied,
        &NeverAbort,
    )
    .unwrap();
    encode_linear_to_host(
        &frame,
        &mut dst_view,
        ColorSpace::Srgb,
        PreMultiplication::UnPreMultiplied,
        &NeverAbort,
    )
    .unwrap();
    assert_eq!(dst_1x1, src_1x1);

    // 3x2 frame with padded + NEGATIVE row_bytes (bottom-up OpenFX layout)
    // Width = 3 px -> 12 bytes active, stride = -20 bytes (8 bytes padding per row)
    let stride: isize = -20;
    let mut raw_src = vec![0xAAu8; 40];
    // Row 0 sits at offset 20..32; Row 1 sits at offset 0..12
    raw_src[20..24].copy_from_slice(&[10, 20, 30, 255]);
    raw_src[24..28].copy_from_slice(&[40, 50, 60, 255]);
    raw_src[28..32].copy_from_slice(&[70, 80, 90, 255]);
    raw_src[0..4].copy_from_slice(&[100, 110, 120, 255]);
    raw_src[4..8].copy_from_slice(&[130, 140, 150, 255]);
    raw_src[8..12].copy_from_slice(&[160, 170, 180, 255]);

    let host_src = HostImageRef::new(&raw_src, 3, 2, PixelFormat::Rgba8, stride, 20).unwrap();
    let mut raw_dst = vec![0x55u8; 48];
    // Destination uses positive padded stride = +24 bytes
    let mut host_dst = HostImageMut::new(&mut raw_dst, 3, 2, PixelFormat::Rgba8, 24, 0).unwrap();

    copy_host_bypass(&host_src, &mut host_dst).unwrap();

    // Row 0 copied to offset 0..12, padding 12..24 untouched
    assert_eq!(&raw_dst[0..12], &raw_src[20..32]);
    assert!(raw_dst[12..24].iter().all(|&b| b == 0x55));
    // Row 1 copied to offset 24..36, padding 36..48 untouched
    assert_eq!(&raw_dst[24..36], &raw_src[0..12]);
    assert!(raw_dst[36..48].iter().all(|&b| b == 0x55));
}

#[test]
fn hostile_8192x8192_small_buffer_rejected_by_stride_bounds_check() {
    let tiny = [0u8; 1024];
    let err = HostImageRef::tightly_packed(&tiny, 8192, 8192, PixelFormat::Rgba8).unwrap_err();
    assert!(matches!(err, CoreError::BufferTooSmall { .. }));
}

#[test]
fn alpha_premultiplication_never_leaks_rgb_on_transparent_pixels() {
    let leak_candidate = Rgba32F::new(1.0, 0.5, 0.25, 0.0);
    assert_eq!(
        leak_candidate.to_straight(PreMultiplication::PreMultiplied),
        Rgba32F::TRANSPARENT
    );
    assert_eq!(
        leak_candidate.to_straight(PreMultiplication::UnPreMultiplied),
        Rgba32F::TRANSPARENT
    );
    assert_eq!(
        leak_candidate.from_straight(PreMultiplication::PreMultiplied),
        Rgba32F::TRANSPARENT
    );
    assert_eq!(leak_candidate.premultiply(), Rgba32F::TRANSPARENT);
}

#[test]
fn safe_math_handles_nan_inf_and_inverted_clamp_bounds() {
    assert_eq!(safe_clamp_f32(f32::NAN, 0.0, 1.0), 0.0);
    assert_eq!(safe_clamp_f32(f32::INFINITY, 0.0, 1.0), 0.0);
    assert_eq!(safe_clamp_f32(0.5, 1.0, 0.0), 0.5);
    assert_eq!(safe_clamp_f64(f64::NEG_INFINITY, -2.0, 2.0), -2.0);
    assert_eq!(safe_div_f32(10.0, 0.0, 42.0), 42.0);
    assert_eq!(safe_div_f32(f32::NAN, 2.0, 7.0), 7.0);
}

#[test]
fn parameter_sanitization_and_validation() {
    let valid = ParamDescriptor::double("gain", "Gain", "Linear gain", 1.0, 0.0, 4.0);
    assert!(valid.validate().is_ok());

    let broken = ParamDescriptor::double("bad", "Bad", "Invalid range", 5.0, 0.0, 4.0);
    assert!(broken.validate().is_err());

    let mut set = ParamSet::defaults_from(&[valid]);
    set.insert("gain", ParamValue::Double(f64::NAN));
    assert_eq!(set.get_double("gain", 1.0, 0.0, 4.0), 1.0);
    set.insert("gain", ParamValue::Double(1_000_000.0));
    assert_eq!(set.get_double("gain", 1.0, 0.0, 4.0), 4.0);
}

#[test]
fn determinism_identical_for_random_access_frames() {
    let k1 = frame_time_to_key(42.0);
    let k2 = frame_time_to_key(42.0);
    assert_eq!(k1, k2);
    assert_eq!(frame_time_to_key(f64::NAN), 0);

    let mut r1 = DeterministicRng::new(k1, 99, 1);
    let mut r2 = DeterministicRng::new(k2, 99, 1);
    for _ in 0..32 {
        assert_eq!(r1.next_u64(), r2.next_u64());
    }
    assert_eq!(hash_2d(10, 20, k1, 7), hash_2d(10, 20, k2, 7));
}

#[test]
fn separable_blur_preserves_constant_field_and_respects_abort() {
    let mut pool = BufferPool::new();
    let mut src = pool.acquire(32, 32).unwrap();
    let fill = Rgba32F::new(0.25, 0.5, 0.75, 0.8);
    for px in src.pixels_mut() {
        *px = fill;
    }

    let mut dst = pool.acquire(32, 32).unwrap();
    let mut scratch = pool.acquire(32, 32).unwrap();
    gaussian_approx_blur(&src, &mut dst, &mut scratch, 4, &NeverAbort).unwrap();

    for px in dst.pixels() {
        assert!((px.r - fill.r).abs() < 1e-4);
        assert!((px.g - fill.g).abs() < 1e-4);
        assert!((px.b - fill.b).abs() < 1e-4);
        assert!((px.a - fill.a).abs() < 1e-4);
    }

    let abort = AtomicAbort::new(true);
    assert_eq!(
        gaussian_approx_blur(&src, &mut dst, &mut scratch, 4, &abort),
        Err(CoreError::Aborted)
    );

    pool.release(src);
    pool.release(dst);
    pool.release(scratch);
    assert_eq!(pool.cached_count(), 3);
}
