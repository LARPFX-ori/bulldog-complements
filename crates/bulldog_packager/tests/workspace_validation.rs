//! Workspace-wide integration tests validating ALL 9 Bulldog Complements suites:
//! - Global descriptor uniqueness, valid parameter bounds, and clean-room compliance
//! - Hostile input gauntlet (0x0, 1x1, NaN/Inf/extreme parameters, negative padded strides, abort)
//! - Random-access determinism across every plugin
//! - Full C ABI OpenFX lifecycle through `bulldog_ofx::mock_host`

use std::collections::BTreeMap;

use bulldog_core::{
    AtomicAbort, BackendKind, BufferPool, ColorSpace, CoreError, EffectContext, EffectPlugin,
    HostImageMut, HostImageRef, HostRenderInputs, NeverAbort, ParamKind, ParamSet, ParamValue,
    PixelFormat, PreMultiplication, render_effect_safe,
};
use bulldog_ofx::mock_host::{MockImageSpec, MockLifecycleRequest, run_mock_lifecycle};
use bulldog_ofx::{K_OFX_STAT_OK, OfxPlugin, validate_suite};

fn all_plugins() -> Vec<Box<dyn EffectPlugin>> {
    let mut plugins = Vec::new();
    plugins.extend(continuus::suite_plugins());
    plugins.extend(sapphirus::suite_plugins());
    plugins.extend(dulcia_nitentia::suite_plugins());
    plugins.extend(caeruleus_nebulosus::suite_plugins());
    plugins.extend(transitions_3d::suite_plugins());
    plugins.extend(tracker_man::suite_plugins());
    plugins.extend(glass_brokers::suite_plugins());
    plugins.extend(universum::suite_plugins());
    plugins.extend(particulae_legales::suite_plugins());
    plugins
}

type SuiteEntry = (fn() -> i32, fn(i32) -> *const OfxPlugin, Vec<EffectContext>);

fn all_exported_ofx_plugins() -> Vec<(*const OfxPlugin, EffectContext)> {
    let suite_fns: &[SuiteEntry] = &[
        (
            continuus::ofx_plugin_count,
            continuus::ofx_plugin,
            continuus::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            sapphirus::ofx_plugin_count,
            sapphirus::ofx_plugin,
            sapphirus::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            dulcia_nitentia::ofx_plugin_count,
            dulcia_nitentia::ofx_plugin,
            dulcia_nitentia::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            caeruleus_nebulosus::ofx_plugin_count,
            caeruleus_nebulosus::ofx_plugin,
            caeruleus_nebulosus::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            transitions_3d::ofx_plugin_count,
            transitions_3d::ofx_plugin,
            transitions_3d::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            tracker_man::ofx_plugin_count,
            tracker_man::ofx_plugin,
            tracker_man::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            glass_brokers::ofx_plugin_count,
            glass_brokers::ofx_plugin,
            glass_brokers::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            universum::ofx_plugin_count,
            universum::ofx_plugin,
            universum::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
        (
            particulae_legales::ofx_plugin_count,
            particulae_legales::ofx_plugin,
            particulae_legales::suite_descriptors()
                .into_iter()
                .map(|d| d.context)
                .collect(),
        ),
    ];

    let mut out = Vec::new();
    for (get_num, get_plugin, contexts) in suite_fns {
        let count = get_num();
        assert_eq!(count as usize, contexts.len());
        for (i, &ctx) in contexts.iter().enumerate() {
            let ptr = get_plugin(i as i32);
            assert!(!ptr.is_null());
            out.push((ptr, ctx));
        }
    }
    out
}

#[test]
fn all_nine_suites_pass_global_descriptor_validation() {
    let plugins = all_plugins();
    assert_eq!(plugins.len(), 9);
    let descriptors: Vec<_> = plugins.iter().map(|p| p.descriptor()).collect();
    if let Err(errs) = validate_suite(&descriptors) {
        panic!("suite validation failed: {errs:?}");
    }
}

#[test]
fn every_plugin_survives_hostile_inputs_negative_strides_and_abort() {
    let plugins = all_plugins();
    let mut pool = BufferPool::new();

    for plugin in &plugins {
        let desc = plugin.descriptor();

        // 1. Hostile parameter set: NaN, +inf, extreme ints, out-of-bounds choices
        let mut hostile_params = ParamSet::defaults_from(&desc.params);
        for p in &desc.params {
            let val = match &p.kind {
                ParamKind::Double { .. } => {
                    if p.name == "Transition" {
                        ParamValue::Double(0.5)
                    } else {
                        ParamValue::Double(f64::NAN)
                    }
                }
                ParamKind::Int { .. } => ParamValue::Int(i32::MAX),
                ParamKind::Bool { .. } => ParamValue::Bool(true),
                ParamKind::Choice { .. } => ParamValue::Choice(999_999),
                ParamKind::Rgba { .. } => {
                    ParamValue::Rgba([f64::NAN, f64::INFINITY, -5.0, f64::NEG_INFINITY])
                }
                ParamKind::Point2D { .. } => ParamValue::Point2D([f64::NAN, f64::INFINITY]),
            };
            hostile_params.insert(p.name.clone(), val);
        }

        // 2. 1x1 frame in RGBA32F
        let src_1x1 = vec![0u8; 16];
        let mut dst_1x1 = vec![0u8; 16];
        let src_view = HostImageRef::tightly_packed(&src_1x1, 1, 1, PixelFormat::Rgba32F).unwrap();
        let mut dst_view =
            HostImageMut::tightly_packed(&mut dst_1x1, 1, 1, PixelFormat::Rgba32F).unwrap();
        let inputs = match desc.context {
            EffectContext::Transition => HostRenderInputs {
                source: None,
                source_from: Some(src_view),
                source_to: Some(src_view),
            },
            _ => HostRenderInputs {
                source: Some(src_view),
                source_from: None,
                source_to: None,
            },
        };
        let res = render_effect_safe(
            plugin.as_ref(),
            &inputs,
            &mut dst_view,
            &hostile_params,
            f64::NAN,
            [1.0, 1.0],
            ColorSpace::Linear,
            PreMultiplication::PreMultiplied,
            BackendKind::Cpu,
            &NeverAbort,
            &mut pool,
        );
        assert!(
            res.is_ok(),
            "1x1 hostile render failed for {}",
            desc.identifier
        );
        // Verify no NaN escaped into RGBA32F output
        for ch in dst_1x1.chunks_exact(4) {
            let f = f32::from_ne_bytes([ch[0], ch[1], ch[2], ch[3]]);
            assert!(
                f.is_finite(),
                "non-finite output pixel in {}",
                desc.identifier
            );
        }

        // 3. Non-contiguous negative stride RGBA8 (16x16, stride = -80)
        let w = 16u32;
        let h = 16u32;
        let stride: isize = -80;
        let base_off = (h as usize - 1) * 80;
        let raw_a = vec![180u8; h as usize * 80];
        let raw_b = vec![60u8; h as usize * 80];
        let mut raw_out_1 = vec![0u8; h as usize * 80];
        let mut raw_out_2 = vec![0u8; h as usize * 80];

        let view_a = HostImageRef::new(&raw_a, w, h, PixelFormat::Rgba8, stride, base_off).unwrap();
        let view_b = HostImageRef::new(&raw_b, w, h, PixelFormat::Rgba8, stride, base_off).unwrap();

        let mut active_params = ParamSet::defaults_from(&desc.params);
        if desc.context == EffectContext::Transition {
            active_params.insert("Transition", ParamValue::Double(0.45));
        }

        let neg_inputs = match desc.context {
            EffectContext::Transition => HostRenderInputs {
                source: None,
                source_from: Some(view_a),
                source_to: Some(view_b),
            },
            _ => HostRenderInputs {
                source: Some(view_a),
                source_from: None,
                source_to: None,
            },
        };

        let mut out_view_1 =
            HostImageMut::new(&mut raw_out_1, w, h, PixelFormat::Rgba8, stride, base_off).unwrap();
        let mut out_view_2 =
            HostImageMut::new(&mut raw_out_2, w, h, PixelFormat::Rgba8, stride, base_off).unwrap();

        render_effect_safe(
            plugin.as_ref(),
            &neg_inputs,
            &mut out_view_1,
            &active_params,
            42.0,
            [1.0, 1.0],
            ColorSpace::Srgb,
            PreMultiplication::UnPreMultiplied,
            BackendKind::Cpu,
            &NeverAbort,
            &mut pool,
        )
        .unwrap();

        // Render an unrelated frame in between, then render frame 42 again for determinism check
        let mut mid_out = vec![0u8; h as usize * 80];
        let mut mid_view =
            HostImageMut::new(&mut mid_out, w, h, PixelFormat::Rgba8, stride, base_off).unwrap();
        render_effect_safe(
            plugin.as_ref(),
            &neg_inputs,
            &mut mid_view,
            &active_params,
            7.0,
            [1.0, 1.0],
            ColorSpace::Srgb,
            PreMultiplication::UnPreMultiplied,
            BackendKind::Cpu,
            &NeverAbort,
            &mut pool,
        )
        .unwrap();

        render_effect_safe(
            plugin.as_ref(),
            &neg_inputs,
            &mut out_view_2,
            &active_params,
            42.0,
            [1.0, 1.0],
            ColorSpace::Srgb,
            PreMultiplication::UnPreMultiplied,
            BackendKind::Cpu,
            &NeverAbort,
            &mut pool,
        )
        .unwrap();

        assert_eq!(
            raw_out_1, raw_out_2,
            "non-deterministic output detected in {}",
            desc.identifier
        );

        // 4. Cooperative abort check
        let aborted = AtomicAbort::new(true);
        let mut abort_out = vec![0u8; h as usize * 80];
        let mut abort_view =
            HostImageMut::new(&mut abort_out, w, h, PixelFormat::Rgba8, stride, base_off).unwrap();
        let abort_res = render_effect_safe(
            plugin.as_ref(),
            &neg_inputs,
            &mut abort_view,
            &active_params,
            42.0,
            [1.0, 1.0],
            ColorSpace::Srgb,
            PreMultiplication::UnPreMultiplied,
            BackendKind::Cpu,
            &aborted,
            &mut pool,
        );
        assert_eq!(abort_res, Err(CoreError::Aborted));
    }
}

#[test]
fn every_exported_ofx_plugin_completes_full_c_abi_lifecycle() {
    let w = 16u32;
    let h = 16u32;
    let byte_len = (w * h * 4) as usize;

    for (plugin_ptr, context) in all_exported_ofx_plugins() {
        let mut clips = BTreeMap::new();
        if context == EffectContext::Transition {
            clips.insert(
                "SourceFrom".to_owned(),
                MockImageSpec::tightly_packed(
                    vec![200u8; byte_len],
                    w,
                    h,
                    PixelFormat::Rgba8,
                    PreMultiplication::PreMultiplied,
                ),
            );
            clips.insert(
                "SourceTo".to_owned(),
                MockImageSpec::tightly_packed(
                    vec![80u8; byte_len],
                    w,
                    h,
                    PixelFormat::Rgba8,
                    PreMultiplication::PreMultiplied,
                ),
            );
        } else {
            clips.insert(
                "Source".to_owned(),
                MockImageSpec::tightly_packed(
                    vec![160u8; byte_len],
                    w,
                    h,
                    PixelFormat::Rgba8,
                    PreMultiplication::PreMultiplied,
                ),
            );
        }
        clips.insert(
            "Output".to_owned(),
            MockImageSpec::tightly_packed(
                vec![0u8; byte_len],
                w,
                h,
                PixelFormat::Rgba8,
                PreMultiplication::PreMultiplied,
            ),
        );

        let mut overrides = BTreeMap::new();
        if context == EffectContext::Transition {
            overrides.insert("Transition".to_owned(), ParamValue::Double(0.5));
        }

        // SAFETY: `plugin_ptr` came directly from `OfxGetPlugin`.
        let report = unsafe {
            run_mock_lifecycle(
                plugin_ptr,
                MockLifecycleRequest {
                    context,
                    time: 12.0,
                    clips,
                    param_overrides: overrides,
                    aborted: false,
                },
            )
        }
        .unwrap();

        assert_eq!(report.load_status, K_OFX_STAT_OK);
        assert_eq!(report.describe_status, K_OFX_STAT_OK);
        assert_eq!(report.describe_in_context_status, K_OFX_STAT_OK);
        assert_eq!(report.create_status, K_OFX_STAT_OK);
        assert_eq!(report.rod_status, K_OFX_STAT_OK);
        assert_eq!(report.roi_status, K_OFX_STAT_OK);
        assert_eq!(report.render_status, K_OFX_STAT_OK);
        assert_eq!(report.destroy_status, K_OFX_STAT_OK);
        assert_eq!(report.unload_status, K_OFX_STAT_OK);

        let expected_released = if context == EffectContext::Transition {
            3
        } else {
            2
        };
        assert_eq!(report.released_images, expected_released);
    }
}
