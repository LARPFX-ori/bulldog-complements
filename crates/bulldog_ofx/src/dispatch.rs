//! Safe OpenFX 1.4 action dispatcher with `catch_unwind` FFI barrier, poisoned-lock
//! recovery, RAII clip image release, and hostile host-buffer validation.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_char, c_double, c_int, c_void};
use core::ptr;
use std::ffi::CString;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock, PoisonError};

use bulldog_core::{
    AbortCheck, BackendKind, BufferPool, ColorSpace, CoreError, EffectContext, EffectDescriptor,
    EffectPlugin, HostImageMut, HostImageRef, HostRenderInputs, IdentityClip, MAX_FRAME_DIMENSION,
    ParamDescriptor, ParamKind, ParamSet, ParamValue, PixelFormat, PreMultiplication, RectD,
    render_effect_safe,
};

use crate::abi::{
    K_OFX_ACTION_CREATE_INSTANCE, K_OFX_ACTION_DESCRIBE, K_OFX_ACTION_DESTROY_INSTANCE,
    K_OFX_ACTION_LOAD, K_OFX_ACTION_UNLOAD, K_OFX_BIT_DEPTH_BYTE, K_OFX_BIT_DEPTH_FLOAT,
    K_OFX_BIT_DEPTH_SHORT, K_OFX_IMAGE_COMPONENT_RGBA,
    K_OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT, K_OFX_IMAGE_EFFECT_ACTION_GET_ROD,
    K_OFX_IMAGE_EFFECT_ACTION_GET_ROI, K_OFX_IMAGE_EFFECT_ACTION_IS_IDENTITY,
    K_OFX_IMAGE_EFFECT_ACTION_RENDER, K_OFX_IMAGE_EFFECT_CONTEXT_FILTER,
    K_OFX_IMAGE_EFFECT_CONTEXT_GENERAL, K_OFX_IMAGE_EFFECT_CONTEXT_GENERATOR,
    K_OFX_IMAGE_EFFECT_CONTEXT_TRANSITION, K_OFX_IMAGE_EFFECT_PROP_CONTEXT,
    K_OFX_IMAGE_EFFECT_PROP_GROUPING, K_OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH,
    K_OFX_IMAGE_EFFECT_PROP_PREMULTIPLICATION, K_OFX_IMAGE_EFFECT_PROP_REGION_OF_DEFINITION,
    K_OFX_IMAGE_EFFECT_PROP_REGION_OF_INTEREST, K_OFX_IMAGE_EFFECT_PROP_RENDER_SCALE,
    K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE, K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_FROM,
    K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_TO, K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_COMPONENTS,
    K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_CONTEXTS, K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS,
    K_OFX_IMAGE_EFFECT_PROP_SUPPORTS_TILES, K_OFX_IMAGE_PROP_BOUNDS, K_OFX_IMAGE_PROP_DATA,
    K_OFX_IMAGE_PROP_ROW_BYTES, K_OFX_OUTPUT_CLIP_NAME, K_OFX_PARAM_PROP_CHOICE_OPTION,
    K_OFX_PARAM_PROP_DEFAULT, K_OFX_PARAM_PROP_DISPLAY_MAX, K_OFX_PARAM_PROP_DISPLAY_MIN,
    K_OFX_PARAM_PROP_HINT, K_OFX_PARAM_PROP_MAX, K_OFX_PARAM_PROP_MIN, K_OFX_PARAM_TYPE_BOOLEAN,
    K_OFX_PARAM_TYPE_CHOICE, K_OFX_PARAM_TYPE_DOUBLE, K_OFX_PARAM_TYPE_DOUBLE2D,
    K_OFX_PARAM_TYPE_INTEGER, K_OFX_PARAM_TYPE_RGBA, K_OFX_PARAMETER_SUITE,
    K_OFX_PROP_INSTANCE_DATA, K_OFX_PROP_LABEL, K_OFX_PROP_LONG_LABEL, K_OFX_PROP_NAME,
    K_OFX_PROP_SHORT_LABEL, K_OFX_PROP_TIME, K_OFX_PROPERTY_SUITE, K_OFX_SIMPLE_SOURCE_CLIP_NAME,
    K_OFX_STAT_ERR_BAD_HANDLE, K_OFX_STAT_ERR_MISSING_HOST_FEATURE, K_OFX_STAT_FAILED,
    K_OFX_STAT_OK, K_OFX_STAT_REPLY_DEFAULT, K_OFX_TRANSITION_SOURCE_FROM_CLIP_NAME,
    K_OFX_TRANSITION_SOURCE_TO_CLIP_NAME, OfxHost, OfxImageClipHandle, OfxImageEffectHandle,
    OfxImageEffectSuiteV1, OfxParamHandle, OfxParamSetHandle, OfxParameterSuiteV1,
    OfxPropertySetHandle, OfxPropertySuiteV1, OfxRectD, OfxStatus,
};

#[derive(Clone, Copy)]
struct HostSuites {
    host_handle: OfxPropertySetHandle,
    fetch_suite: unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int) -> *const c_void,
    prop: Option<OfxPropertySuiteV1>,
    image_effect: Option<OfxImageEffectSuiteV1>,
    param: Option<OfxParameterSuiteV1>,
}

// SAFETY: OpenFX hosts guarantee host suite function tables remain valid across threads
// between `setHost`/`OfxActionLoad` and `OfxActionUnload`.
unsafe impl Send for HostSuites {}
// SAFETY: Suite tables are read-only C function-pointer structs.
unsafe impl Sync for HostSuites {}

impl HostSuites {
    fn fetch_all(&mut self) -> Result<(), OfxStatus> {
        // SAFETY: `self.fetch_suite` and `self.host_handle` were provided by the OpenFX host
        // during `setHost` and remain valid until `OfxActionUnload`.
        let prop_ptr = unsafe {
            (self.fetch_suite)(self.host_handle, K_OFX_PROPERTY_SUITE.as_ptr(), 1)
                as *const OfxPropertySuiteV1
        };
        let ie_ptr = unsafe {
            (self.fetch_suite)(
                self.host_handle,
                crate::abi::K_OFX_IMAGE_EFFECT_SUITE.as_ptr(),
                1,
            ) as *const OfxImageEffectSuiteV1
        };
        let param_ptr = unsafe {
            (self.fetch_suite)(self.host_handle, K_OFX_PARAMETER_SUITE.as_ptr(), 1)
                as *const OfxParameterSuiteV1
        };
        if prop_ptr.is_null() || ie_ptr.is_null() || param_ptr.is_null() {
            return Err(K_OFX_STAT_ERR_MISSING_HOST_FEATURE);
        }
        // SAFETY: Non-null suite pointers returned by `fetchSuite` match the OpenFX 1.4 C ABI.
        self.prop = Some(unsafe { *prop_ptr });
        self.image_effect = Some(unsafe { *ie_ptr });
        self.param = Some(unsafe { *param_ptr });
        Ok(())
    }
}

struct InstanceState {
    pool: Mutex<BufferPool>,
}

/// Static slot connecting one Rust [`EffectPlugin`] factory to its OpenFX C ABI callbacks.
pub struct PluginSlot {
    factory: fn() -> Box<dyn EffectPlugin>,
    plugin: OnceLock<Box<dyn EffectPlugin>>,
    descriptor: OnceLock<EffectDescriptor>,
    host: Mutex<Option<HostSuites>>,
}

impl core::fmt::Debug for PluginSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PluginSlot").finish_non_exhaustive()
    }
}

impl PluginSlot {
    /// Constructs a `const`-initializable plugin slot for static export tables.
    #[must_use]
    pub const fn new(factory: fn() -> Box<dyn EffectPlugin>) -> Self {
        Self {
            factory,
            plugin: OnceLock::new(),
            descriptor: OnceLock::new(),
            host: Mutex::new(None),
        }
    }

    fn plugin(&self) -> &dyn EffectPlugin {
        self.plugin.get_or_init(self.factory).as_ref()
    }

    fn descriptor(&self) -> &EffectDescriptor {
        self.descriptor.get_or_init(|| self.plugin().descriptor())
    }

    /// Safe `setHost` handler wrapped in `catch_unwind`.
    ///
    /// # Safety
    /// `host_ptr` must be null or a valid pointer to an OpenFX `OfxHost` struct.
    pub unsafe fn handle_set_host(&self, host_ptr: *mut OfxHost) {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            if host_ptr.is_null() {
                return;
            }
            // SAFETY: Host passes a valid pointer to `OfxHost` during `setHost`.
            let host = unsafe { *host_ptr };
            let Some(fetch_suite) = host.fetch_suite else {
                return;
            };
            let mut guard = self.host.lock().unwrap_or_else(PoisonError::into_inner);
            *guard = Some(HostSuites {
                host_handle: host.host,
                fetch_suite,
                prop: None,
                image_effect: None,
                param: None,
            });
        }));
    }

    /// Safe `mainEntry` handler wrapped in `catch_unwind` so panics never cross FFI (§0.7).
    ///
    /// # Safety
    /// Pointers must conform to the OpenFX 1.4 `OfxPluginEntryPoint` contract.
    pub unsafe fn handle_main_entry(
        &self,
        action: *const c_char,
        handle: *const c_void,
        in_args: OfxPropertySetHandle,
        out_args: OfxPropertySetHandle,
    ) -> OfxStatus {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.dispatch_action(action, handle as OfxImageEffectHandle, in_args, out_args)
        }));
        result.unwrap_or(K_OFX_STAT_FAILED)
    }

    fn dispatch_action(
        &self,
        action_ptr: *const c_char,
        handle: OfxImageEffectHandle,
        in_args: OfxPropertySetHandle,
        out_args: OfxPropertySetHandle,
    ) -> OfxStatus {
        if action_ptr.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        // SAFETY: OpenFX host passes a NUL-terminated ASCII action string.
        let Ok(action) = unsafe { CStr::from_ptr(action_ptr) }.to_str() else {
            return K_OFX_STAT_REPLY_DEFAULT;
        };

        match action {
            K_OFX_ACTION_LOAD => self.on_load(),
            K_OFX_ACTION_UNLOAD => self.on_unload(),
            K_OFX_ACTION_DESCRIBE => self.on_describe(handle),
            K_OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT => {
                self.on_describe_in_context(handle, in_args)
            }
            K_OFX_ACTION_CREATE_INSTANCE => self.on_create_instance(handle),
            K_OFX_ACTION_DESTROY_INSTANCE => self.on_destroy_instance(handle),
            K_OFX_IMAGE_EFFECT_ACTION_GET_ROD => self.on_get_rod(handle, in_args, out_args),
            K_OFX_IMAGE_EFFECT_ACTION_GET_ROI => self.on_get_roi(handle, in_args, out_args),
            K_OFX_IMAGE_EFFECT_ACTION_IS_IDENTITY => self.on_is_identity(handle, in_args, out_args),
            K_OFX_IMAGE_EFFECT_ACTION_RENDER => self.on_render(handle, in_args),
            _ => K_OFX_STAT_REPLY_DEFAULT,
        }
    }

    fn suites(
        &self,
    ) -> Result<
        (
            OfxPropertySuiteV1,
            OfxImageEffectSuiteV1,
            OfxParameterSuiteV1,
        ),
        OfxStatus,
    > {
        let mut guard = self.host.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(host) = guard.as_mut() else {
            return Err(K_OFX_STAT_ERR_MISSING_HOST_FEATURE);
        };
        if host.prop.is_none() || host.image_effect.is_none() || host.param.is_none() {
            host.fetch_all()?;
        }
        match (host.prop, host.image_effect, host.param) {
            (Some(p), Some(ie), Some(pr)) => Ok((p, ie, pr)),
            _ => Err(K_OFX_STAT_ERR_MISSING_HOST_FEATURE),
        }
    }

    fn on_load(&self) -> OfxStatus {
        let _ = self.descriptor();
        match self.suites() {
            Ok(_) => K_OFX_STAT_OK,
            Err(status) => status,
        }
    }

    fn on_unload(&self) -> OfxStatus {
        let mut guard = self.host.lock().unwrap_or_else(PoisonError::into_inner);
        *guard = None;
        K_OFX_STAT_OK
    }

    fn on_describe(&self, handle: OfxImageEffectHandle) -> OfxStatus {
        if handle.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, _param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };
        let Some(props) = get_effect_props(&ie, handle) else {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        };

        let desc = self.descriptor();
        set_str(&prop, props, K_OFX_PROP_LABEL, 0, &desc.label);
        set_str(&prop, props, K_OFX_PROP_SHORT_LABEL, 0, &desc.label);
        set_str(&prop, props, K_OFX_PROP_LONG_LABEL, 0, &desc.label);
        set_str(
            &prop,
            props,
            K_OFX_IMAGE_EFFECT_PROP_GROUPING,
            0,
            &desc.grouping,
        );

        let ctx_cstr = match desc.context {
            EffectContext::Filter => K_OFX_IMAGE_EFFECT_CONTEXT_FILTER,
            EffectContext::Transition => K_OFX_IMAGE_EFFECT_CONTEXT_TRANSITION,
            EffectContext::Generator => K_OFX_IMAGE_EFFECT_CONTEXT_GENERATOR,
            EffectContext::General => K_OFX_IMAGE_EFFECT_CONTEXT_GENERAL,
        };
        set_cstr(
            &prop,
            props,
            K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_CONTEXTS,
            0,
            ctx_cstr,
        );

        set_cstr(
            &prop,
            props,
            K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS,
            0,
            K_OFX_BIT_DEPTH_BYTE,
        );
        set_cstr(
            &prop,
            props,
            K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS,
            1,
            K_OFX_BIT_DEPTH_SHORT,
        );
        set_cstr(
            &prop,
            props,
            K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS,
            2,
            K_OFX_BIT_DEPTH_FLOAT,
        );
        set_int(&prop, props, K_OFX_IMAGE_EFFECT_PROP_SUPPORTS_TILES, 0, 0);

        K_OFX_STAT_OK
    }

    fn on_describe_in_context(
        &self,
        handle: OfxImageEffectHandle,
        in_args: OfxPropertySetHandle,
    ) -> OfxStatus {
        if handle.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };
        let desc = self.descriptor();

        // Check requested context string if supplied.
        let requested_ctx = get_string(&prop, in_args, K_OFX_IMAGE_EFFECT_PROP_CONTEXT, 0);
        let is_transition = desc.context == EffectContext::Transition
            || requested_ctx.as_deref() == Some("OfxImageEffectContextTransition");

        // Define clips according to context.
        if is_transition {
            define_rgba_clip(&prop, &ie, handle, K_OFX_TRANSITION_SOURCE_FROM_CLIP_NAME);
            define_rgba_clip(&prop, &ie, handle, K_OFX_TRANSITION_SOURCE_TO_CLIP_NAME);
        } else if desc.context != EffectContext::Generator {
            define_rgba_clip(&prop, &ie, handle, K_OFX_SIMPLE_SOURCE_CLIP_NAME);
        }
        define_rgba_clip(&prop, &ie, handle, K_OFX_OUTPUT_CLIP_NAME);

        // Define parameters on the effect's parameter set.
        let Some(param_set) = get_param_set(&ie, handle) else {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        };
        for p in &desc.params {
            define_parameter(&prop, &param, param_set, p);
        }

        K_OFX_STAT_OK
    }

    fn on_create_instance(&self, handle: OfxImageEffectHandle) -> OfxStatus {
        if handle.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, _param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };
        let Some(props) = get_effect_props(&ie, handle) else {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        };
        let state = Box::new(InstanceState {
            pool: Mutex::new(BufferPool::new()),
        });
        let raw_ptr = Box::into_raw(state).cast::<c_void>();
        set_ptr(&prop, props, K_OFX_PROP_INSTANCE_DATA, 0, raw_ptr);
        K_OFX_STAT_OK
    }

    fn on_destroy_instance(&self, handle: OfxImageEffectHandle) -> OfxStatus {
        if handle.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, _param)) = self.suites() else {
            return K_OFX_STAT_OK;
        };
        let Some(props) = get_effect_props(&ie, handle) else {
            return K_OFX_STAT_OK;
        };
        if let Some(raw_ptr) = get_ptr(&prop, props, K_OFX_PROP_INSTANCE_DATA, 0)
            && !raw_ptr.is_null()
        {
            set_ptr(&prop, props, K_OFX_PROP_INSTANCE_DATA, 0, ptr::null_mut());
            // SAFETY: Pointer was allocated via `Box::into_raw` in `on_create_instance` and
            // cleared above before reclamation to prevent double-free.
            unsafe {
                drop(Box::from_raw(raw_ptr.cast::<InstanceState>()));
            }
        }
        K_OFX_STAT_OK
    }

    fn on_get_rod(
        &self,
        handle: OfxImageEffectHandle,
        in_args: OfxPropertySetHandle,
        out_args: OfxPropertySetHandle,
    ) -> OfxStatus {
        if handle.is_null() || out_args.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };
        let time = get_double(&prop, in_args, K_OFX_PROP_TIME, 0).unwrap_or(0.0);
        let desc = self.descriptor();
        let params = read_params(&prop, &ie, &param, handle, desc, time);

        let src_rod = if desc.context == EffectContext::Transition {
            let r_from = get_clip_rod(&ie, handle, K_OFX_TRANSITION_SOURCE_FROM_CLIP_NAME, time)
                .unwrap_or_default();
            let r_to = get_clip_rod(&ie, handle, K_OFX_TRANSITION_SOURCE_TO_CLIP_NAME, time)
                .unwrap_or_default();
            r_from.union(r_to)
        } else {
            get_clip_rod(&ie, handle, K_OFX_SIMPLE_SOURCE_CLIP_NAME, time).unwrap_or_default()
        };

        let out_rod = self.plugin().region_of_definition(src_rod, &params, time);
        set_rect_d(
            &prop,
            out_args,
            K_OFX_IMAGE_EFFECT_PROP_REGION_OF_DEFINITION,
            out_rod,
        );
        K_OFX_STAT_OK
    }

    fn on_get_roi(
        &self,
        handle: OfxImageEffectHandle,
        in_args: OfxPropertySetHandle,
        out_args: OfxPropertySetHandle,
    ) -> OfxStatus {
        if handle.is_null() || out_args.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };
        let time = get_double(&prop, in_args, K_OFX_PROP_TIME, 0).unwrap_or(0.0);
        let desc = self.descriptor();
        let params = read_params(&prop, &ie, &param, handle, desc, time);

        let requested = get_rect_d(&prop, in_args, K_OFX_IMAGE_EFFECT_PROP_REGION_OF_INTEREST)
            .unwrap_or_default();
        let roi = self.plugin().regions_of_interest(requested, &params, time);

        if desc.context == EffectContext::Transition {
            set_rect_d(
                &prop,
                out_args,
                K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_FROM,
                roi,
            );
            set_rect_d(&prop, out_args, K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_TO, roi);
        } else if desc.context != EffectContext::Generator {
            set_rect_d(&prop, out_args, K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE, roi);
        }
        K_OFX_STAT_OK
    }

    fn on_is_identity(
        &self,
        handle: OfxImageEffectHandle,
        in_args: OfxPropertySetHandle,
        out_args: OfxPropertySetHandle,
    ) -> OfxStatus {
        if handle.is_null() || out_args.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };
        let time = get_double(&prop, in_args, K_OFX_PROP_TIME, 0).unwrap_or(0.0);
        let desc = self.descriptor();
        let params = read_params(&prop, &ie, &param, handle, desc, time);

        if let Some(clip) = self.plugin().is_identity(&params, time) {
            let clip_name = match clip {
                IdentityClip::Source => K_OFX_SIMPLE_SOURCE_CLIP_NAME,
                IdentityClip::SourceFrom => K_OFX_TRANSITION_SOURCE_FROM_CLIP_NAME,
                IdentityClip::SourceTo => K_OFX_TRANSITION_SOURCE_TO_CLIP_NAME,
            };
            set_cstr(&prop, out_args, K_OFX_PROP_NAME, 0, clip_name);
            set_double(&prop, out_args, K_OFX_PROP_TIME, 0, time);
            K_OFX_STAT_OK
        } else {
            K_OFX_STAT_REPLY_DEFAULT
        }
    }

    fn on_render(&self, handle: OfxImageEffectHandle, in_args: OfxPropertySetHandle) -> OfxStatus {
        if handle.is_null() {
            return K_OFX_STAT_ERR_BAD_HANDLE;
        }
        let Ok((prop, ie, param)) = self.suites() else {
            return K_OFX_STAT_ERR_MISSING_HOST_FEATURE;
        };

        let abort_check = OfxAbortCheck {
            suite: ie,
            effect: handle,
        };
        if abort_check.is_aborted() {
            return K_OFX_STAT_OK;
        }

        let time = get_double(&prop, in_args, K_OFX_PROP_TIME, 0).unwrap_or(0.0);
        let sx = get_double(&prop, in_args, K_OFX_IMAGE_EFFECT_PROP_RENDER_SCALE, 0).unwrap_or(1.0);
        let sy = get_double(&prop, in_args, K_OFX_IMAGE_EFFECT_PROP_RENDER_SCALE, 1).unwrap_or(1.0);

        let desc = self.descriptor();
        let params = read_params(&prop, &ie, &param, handle, desc, time);

        // Fetch RAII-guarded output image (always released on scope exit).
        let Some(dst_guard) = fetch_clip_image(&ie, handle, K_OFX_OUTPUT_CLIP_NAME, time) else {
            return K_OFX_STAT_FAILED;
        };
        let src_guard = if desc.context != EffectContext::Transition
            && desc.context != EffectContext::Generator
        {
            fetch_clip_image(&ie, handle, K_OFX_SIMPLE_SOURCE_CLIP_NAME, time)
        } else {
            None
        };
        let from_guard = if desc.context == EffectContext::Transition {
            fetch_clip_image(&ie, handle, K_OFX_TRANSITION_SOURCE_FROM_CLIP_NAME, time)
        } else {
            None
        };
        let to_guard = if desc.context == EffectContext::Transition {
            fetch_clip_image(&ie, handle, K_OFX_TRANSITION_SOURCE_TO_CLIP_NAME, time)
        } else {
            None
        };

        let Some(dst_raw) = extract_raw_image(&prop, dst_guard.handle) else {
            return K_OFX_STAT_FAILED;
        };
        let src_raw = src_guard
            .as_ref()
            .and_then(|g| extract_raw_image(&prop, g.handle));
        let from_raw = from_guard
            .as_ref()
            .and_then(|g| extract_raw_image(&prop, g.handle));
        let to_raw = to_guard
            .as_ref()
            .and_then(|g| extract_raw_image(&prop, g.handle));

        let premult = dst_raw.premult;
        let color_space = match dst_raw.format {
            PixelFormat::Rgba32F => ColorSpace::Linear,
            PixelFormat::Rgba8 | PixelFormat::Rgba16 => ColorSpace::Srgb,
        };

        let Ok(mut dst_mut) = raw_to_host_mut(&dst_raw) else {
            return K_OFX_STAT_FAILED;
        };
        let src_ref = src_raw.as_ref().and_then(|r| raw_to_host_ref(r).ok());
        let from_ref = from_raw.as_ref().and_then(|r| raw_to_host_ref(r).ok());
        let to_ref = to_raw.as_ref().and_then(|r| raw_to_host_ref(r).ok());

        let inputs = HostRenderInputs {
            source: src_ref,
            source_from: from_ref,
            source_to: to_ref,
        };

        let mut fallback_pool = BufferPool::new();
        let instance_state_ptr = get_effect_props(&ie, handle)
            .and_then(|p| get_ptr(&prop, p, K_OFX_PROP_INSTANCE_DATA, 0))
            .filter(|p| !p.is_null());

        let render_result = if let Some(ptr) = instance_state_ptr {
            // SAFETY: Pointer was created in `on_create_instance` and remains valid until
            // `on_destroy_instance`.
            let state = unsafe { &*ptr.cast::<InstanceState>() };
            let mut pool = state.pool.lock().unwrap_or_else(PoisonError::into_inner);
            render_effect_safe(
                self.plugin(),
                &inputs,
                &mut dst_mut,
                &params,
                time,
                [sx, sy],
                color_space,
                premult,
                BackendKind::Cpu,
                &abort_check,
                &mut pool,
            )
        } else {
            render_effect_safe(
                self.plugin(),
                &inputs,
                &mut dst_mut,
                &params,
                time,
                [sx, sy],
                color_space,
                premult,
                BackendKind::Cpu,
                &abort_check,
                &mut fallback_pool,
            )
        };

        match render_result {
            Ok(_) | Err(CoreError::Aborted) => K_OFX_STAT_OK,
            Err(_) => K_OFX_STAT_FAILED,
        }
    }
}

struct OfxAbortCheck {
    suite: OfxImageEffectSuiteV1,
    effect: OfxImageEffectHandle,
}

// SAFETY: OpenFX `abort` callback is explicitly specified as thread-safe during render.
unsafe impl Sync for OfxAbortCheck {}

impl AbortCheck for OfxAbortCheck {
    fn is_aborted(&self) -> bool {
        let Some(abort_fn) = self.suite.abort else {
            return false;
        };
        // SAFETY: `self.effect` is the active render instance handle.
        unsafe { abort_fn(self.effect) != 0 }
    }
}

struct ClipImageGuard {
    suite: OfxImageEffectSuiteV1,
    handle: OfxPropertySetHandle,
}

impl Drop for ClipImageGuard {
    fn drop(&mut self) {
        if !self.handle.is_null()
            && let Some(release) = self.suite.clip_release_image
        {
            // SAFETY: Releases image handle fetched via `clip_get_image` exactly once.
            let _ = unsafe { release(self.handle) };
        }
    }
}

struct RawImageDesc {
    base_ptr: *mut u8,
    byte_len: usize,
    base_offset: usize,
    width: u32,
    height: u32,
    row_bytes: isize,
    format: PixelFormat,
    premult: PreMultiplication,
}

fn extract_raw_image(
    prop: &OfxPropertySuiteV1,
    img_props: OfxPropertySetHandle,
) -> Option<RawImageDesc> {
    let data_ptr = get_ptr(prop, img_props, K_OFX_IMAGE_PROP_DATA, 0)?;
    if data_ptr.is_null() {
        return None;
    }
    let x1 = get_int(prop, img_props, K_OFX_IMAGE_PROP_BOUNDS, 0)?;
    let y1 = get_int(prop, img_props, K_OFX_IMAGE_PROP_BOUNDS, 1)?;
    let x2 = get_int(prop, img_props, K_OFX_IMAGE_PROP_BOUNDS, 2)?;
    let y2 = get_int(prop, img_props, K_OFX_IMAGE_PROP_BOUNDS, 3)?;

    let w_i = x2.checked_sub(x1)?;
    let h_i = y2.checked_sub(y1)?;
    if w_i <= 0 || h_i <= 0 {
        return None;
    }
    let width = u32::try_from(w_i).ok()?;
    let height = u32::try_from(h_i).ok()?;
    if width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION {
        return None;
    }

    let row_bytes_i32 = get_int(prop, img_props, K_OFX_IMAGE_PROP_ROW_BYTES, 0)?;
    let row_bytes = isize::try_from(row_bytes_i32).ok()?;

    let depth_str = get_string(prop, img_props, K_OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH, 0)?;
    let format = match depth_str.as_str() {
        "OfxBitDepthByte" => PixelFormat::Rgba8,
        "OfxBitDepthShort" => PixelFormat::Rgba16,
        "OfxBitDepthFloat" => PixelFormat::Rgba32F,
        _ => return None,
    };

    let premult_str = get_string(
        prop,
        img_props,
        K_OFX_IMAGE_EFFECT_PROP_PREMULTIPLICATION,
        0,
    );
    let premult = match premult_str.as_deref() {
        Some("OfxImageOpaque") => PreMultiplication::Opaque,
        Some("OfxImageUnPreMultiplied") => PreMultiplication::UnPreMultiplied,
        _ => PreMultiplication::PreMultiplied,
    };

    let min_row_bytes = (width as usize).checked_mul(format.bytes_per_pixel())?;
    let abs_stride = row_bytes.unsigned_abs();
    if abs_stride < min_row_bytes {
        return None;
    }

    let last_row = (height as usize).saturating_sub(1);
    let span = last_row.checked_mul(abs_stride)?;
    let byte_len = span.checked_add(min_row_bytes)?;

    // In OpenFX, `OfxImagePropData` points to row 0 (bottom-left pixel).
    // When `row_bytes < 0`, row 0 sits at the highest address and subsequent rows walk
    // backwards by `abs_stride`, so the contiguous backing memory starts at `data_ptr - span`.
    let (base_ptr, base_offset) = if row_bytes >= 0 {
        (data_ptr.cast::<u8>(), 0)
    } else {
        let span_isize = isize::try_from(span).ok()?;
        // SAFETY: Host guarantees the image buffer spans `height` rows with `row_bytes` stride
        // starting at `data_ptr` for row 0 and stepping by `row_bytes` for each row `0..height`.
        let low_ptr = unsafe { data_ptr.cast::<u8>().offset(-span_isize) };
        (low_ptr, span)
    };

    Some(RawImageDesc {
        base_ptr,
        byte_len,
        base_offset,
        width,
        height,
        row_bytes,
        format,
        premult,
    })
}

fn raw_to_host_ref<'a>(raw: &RawImageDesc) -> Result<HostImageRef<'a>, CoreError> {
    if raw.base_ptr.is_null() {
        return Err(CoreError::Internal(
            "null host source image buffer pointer".to_owned(),
        ));
    }
    // SAFETY: Validated non-null host buffer spanning `raw.byte_len` bytes for the duration
    // of the active `OfxImageEffectActionRender` call while `ClipImageGuard` holds the image.
    let slice = unsafe { core::slice::from_raw_parts(raw.base_ptr, raw.byte_len) };
    HostImageRef::new(
        slice,
        raw.width,
        raw.height,
        raw.format,
        raw.row_bytes,
        raw.base_offset,
    )
}

fn raw_to_host_mut<'a>(raw: &RawImageDesc) -> Result<HostImageMut<'a>, CoreError> {
    if raw.base_ptr.is_null() {
        return Err(CoreError::Internal(
            "null host output image buffer pointer".to_owned(),
        ));
    }
    // SAFETY: Output image buffer is uniquely owned by this render call until `clipReleaseImage`.
    let slice = unsafe { core::slice::from_raw_parts_mut(raw.base_ptr, raw.byte_len) };
    HostImageMut::new(
        slice,
        raw.width,
        raw.height,
        raw.format,
        raw.row_bytes,
        raw.base_offset,
    )
}

fn fetch_clip_image(
    ie: &OfxImageEffectSuiteV1,
    effect: OfxImageEffectHandle,
    clip_name: &CStr,
    time: f64,
) -> Option<ClipImageGuard> {
    let get_clip = ie.clip_get_handle?;
    let get_img = ie.clip_get_image?;
    let mut clip: OfxImageClipHandle = ptr::null_mut();
    // SAFETY: Passes valid clip name and out-pointer to OpenFX host.
    let st = unsafe { get_clip(effect, clip_name.as_ptr(), &mut clip, ptr::null_mut()) };
    if st != K_OFX_STAT_OK || clip.is_null() {
        return None;
    }
    let mut img_props: OfxPropertySetHandle = ptr::null_mut();
    // SAFETY: Fetches full-clip image at `time` from a valid clip handle.
    let st = unsafe { get_img(clip, time, ptr::null(), &mut img_props) };
    if st != K_OFX_STAT_OK || img_props.is_null() {
        return None;
    }
    Some(ClipImageGuard {
        suite: *ie,
        handle: img_props,
    })
}

fn get_clip_rod(
    ie: &OfxImageEffectSuiteV1,
    effect: OfxImageEffectHandle,
    clip_name: &CStr,
    time: f64,
) -> Option<RectD> {
    let get_clip = ie.clip_get_handle?;
    let get_rod = ie.clip_get_region_of_definition?;
    let mut clip: OfxImageClipHandle = ptr::null_mut();
    // SAFETY: Valid effect handle and NUL-terminated clip name.
    let st = unsafe { get_clip(effect, clip_name.as_ptr(), &mut clip, ptr::null_mut()) };
    if st != K_OFX_STAT_OK || clip.is_null() {
        return None;
    }
    let mut rod = OfxRectD::default();
    // SAFETY: Valid clip handle and initialized `OfxRectD` out-pointer.
    let st = unsafe { get_rod(clip, time, &mut rod) };
    if st != K_OFX_STAT_OK {
        return None;
    }
    Some(RectD::new(rod.x1, rod.y1, rod.x2, rod.y2))
}

fn define_rgba_clip(
    prop: &OfxPropertySuiteV1,
    ie: &OfxImageEffectSuiteV1,
    effect: OfxImageEffectHandle,
    clip_name: &CStr,
) {
    let Some(define) = ie.clip_define else {
        return;
    };
    let mut clip_props: OfxPropertySetHandle = ptr::null_mut();
    // SAFETY: Calls host `clipDefine` with valid effect handle and NUL-terminated name.
    let st = unsafe { define(effect, clip_name.as_ptr(), &mut clip_props) };
    if st == K_OFX_STAT_OK && !clip_props.is_null() {
        set_cstr(
            prop,
            clip_props,
            K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_COMPONENTS,
            0,
            K_OFX_IMAGE_COMPONENT_RGBA,
        );
    }
}

fn define_parameter(
    prop: &OfxPropertySuiteV1,
    param: &OfxParameterSuiteV1,
    param_set: OfxParamSetHandle,
    desc: &ParamDescriptor,
) {
    let Some(define) = param.param_define else {
        return;
    };
    let Ok(name_c) = CString::new(desc.name.as_str()) else {
        return;
    };
    let type_cstr = match &desc.kind {
        ParamKind::Double { .. } => K_OFX_PARAM_TYPE_DOUBLE,
        ParamKind::Int { .. } => K_OFX_PARAM_TYPE_INTEGER,
        ParamKind::Bool { .. } => K_OFX_PARAM_TYPE_BOOLEAN,
        ParamKind::Choice { .. } => K_OFX_PARAM_TYPE_CHOICE,
        ParamKind::Rgba { .. } => K_OFX_PARAM_TYPE_RGBA,
        ParamKind::Point2D { .. } => K_OFX_PARAM_TYPE_DOUBLE2D,
    };

    let mut p_props: OfxPropertySetHandle = ptr::null_mut();
    // SAFETY: Calls host `paramDefine` with valid param set and NUL-terminated strings.
    let st = unsafe { define(param_set, type_cstr.as_ptr(), name_c.as_ptr(), &mut p_props) };
    if st != K_OFX_STAT_OK || p_props.is_null() {
        return;
    }

    set_str(prop, p_props, K_OFX_PROP_LABEL, 0, &desc.label);
    set_str(prop, p_props, K_OFX_PARAM_PROP_HINT, 0, &desc.hint);

    match &desc.kind {
        ParamKind::Double {
            default,
            min,
            max,
            display_min,
            display_max,
        } => {
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 0, *default);
            set_double(prop, p_props, K_OFX_PARAM_PROP_MIN, 0, *min);
            set_double(prop, p_props, K_OFX_PARAM_PROP_MAX, 0, *max);
            set_double(prop, p_props, K_OFX_PARAM_PROP_DISPLAY_MIN, 0, *display_min);
            set_double(prop, p_props, K_OFX_PARAM_PROP_DISPLAY_MAX, 0, *display_max);
        }
        ParamKind::Int { default, min, max } => {
            set_int(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 0, *default);
            set_int(prop, p_props, K_OFX_PARAM_PROP_MIN, 0, *min);
            set_int(prop, p_props, K_OFX_PARAM_PROP_MAX, 0, *max);
            set_int(prop, p_props, K_OFX_PARAM_PROP_DISPLAY_MIN, 0, *min);
            set_int(prop, p_props, K_OFX_PARAM_PROP_DISPLAY_MAX, 0, *max);
        }
        ParamKind::Bool { default } => {
            set_int(
                prop,
                p_props,
                K_OFX_PARAM_PROP_DEFAULT,
                0,
                c_int::from(*default),
            );
        }
        ParamKind::Choice { default, options } => {
            for (idx, opt) in options.iter().enumerate() {
                if let Ok(i) = c_int::try_from(idx) {
                    set_str(prop, p_props, K_OFX_PARAM_PROP_CHOICE_OPTION, i, opt);
                }
            }
            if let Ok(d) = c_int::try_from(*default) {
                set_int(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 0, d);
            }
        }
        ParamKind::Rgba { default } => {
            let [r, g, b, a] = *default;
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 0, r);
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 1, g);
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 2, b);
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 3, a);
        }
        ParamKind::Point2D { default, min, max } => {
            let [dx, dy] = *default;
            let [lo_x, lo_y] = *min;
            let [hi_x, hi_y] = *max;
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 0, dx);
            set_double(prop, p_props, K_OFX_PARAM_PROP_DEFAULT, 1, dy);
            set_double(prop, p_props, K_OFX_PARAM_PROP_MIN, 0, lo_x);
            set_double(prop, p_props, K_OFX_PARAM_PROP_MIN, 1, lo_y);
            set_double(prop, p_props, K_OFX_PARAM_PROP_MAX, 0, hi_x);
            set_double(prop, p_props, K_OFX_PARAM_PROP_MAX, 1, hi_y);
        }
    }
}

fn read_params(
    prop: &OfxPropertySuiteV1,
    ie: &OfxImageEffectSuiteV1,
    param: &OfxParameterSuiteV1,
    effect: OfxImageEffectHandle,
    desc: &EffectDescriptor,
    time: f64,
) -> ParamSet {
    let mut set = ParamSet::defaults_from(&desc.params);
    let Some(param_set) = get_param_set(ie, effect) else {
        return set;
    };
    let Some(get_handle) = param.param_get_handle else {
        return set;
    };

    for p in &desc.params {
        let Ok(name_c) = CString::new(p.name.as_str()) else {
            continue;
        };
        let mut handle: OfxParamHandle = ptr::null_mut();
        let mut param_props: OfxPropertySetHandle = ptr::null_mut();
        // SAFETY: Fetches parameter handle and property set by NUL-terminated name.
        let st = unsafe { get_handle(param_set, name_c.as_ptr(), &mut handle, &mut param_props) };
        if st != K_OFX_STAT_OK || handle.is_null() {
            continue;
        }
        if let Some(get_val_at_time) = param.param_get_value_at_time {
            match &p.kind {
                ParamKind::Double { .. } => {
                    let mut val: c_double = 0.0;
                    // SAFETY: OpenFX `Double` parameter writes one `double` via variadic out-arg.
                    if unsafe { get_val_at_time(handle, time, &mut val as *mut c_double) }
                        == K_OFX_STAT_OK
                    {
                        set.insert(p.name.clone(), ParamValue::Double(val));
                    }
                }
                ParamKind::Int { .. } => {
                    let mut val: c_int = 0;
                    // SAFETY: OpenFX `Integer` parameter writes one `int` via variadic out-arg.
                    if unsafe { get_val_at_time(handle, time, &mut val as *mut c_int) }
                        == K_OFX_STAT_OK
                    {
                        set.insert(p.name.clone(), ParamValue::Int(val));
                    }
                }
                ParamKind::Bool { .. } => {
                    let mut val: c_int = 0;
                    // SAFETY: OpenFX `Boolean` parameter writes one `int` (0 or 1).
                    if unsafe { get_val_at_time(handle, time, &mut val as *mut c_int) }
                        == K_OFX_STAT_OK
                    {
                        set.insert(p.name.clone(), ParamValue::Bool(val != 0));
                    }
                }
                ParamKind::Choice { .. } => {
                    let mut val: c_int = 0;
                    // SAFETY: OpenFX `Choice` parameter writes one 0-based `int` index.
                    if unsafe { get_val_at_time(handle, time, &mut val as *mut c_int) }
                        == K_OFX_STAT_OK
                        && let Ok(idx) = usize::try_from(val)
                    {
                        set.insert(p.name.clone(), ParamValue::Choice(idx));
                    }
                }
                ParamKind::Rgba { .. } => {
                    let mut r: c_double = 0.0;
                    let mut g: c_double = 0.0;
                    let mut b: c_double = 0.0;
                    let mut a: c_double = 1.0;
                    // SAFETY: OpenFX `RGBA` parameter writes four `double` out-arguments.
                    if unsafe {
                        get_val_at_time(
                            handle,
                            time,
                            &mut r as *mut c_double,
                            &mut g as *mut c_double,
                            &mut b as *mut c_double,
                            &mut a as *mut c_double,
                        )
                    } == K_OFX_STAT_OK
                    {
                        set.insert(p.name.clone(), ParamValue::Rgba([r, g, b, a]));
                    }
                }
                ParamKind::Point2D { .. } => {
                    let mut x: c_double = 0.0;
                    let mut y: c_double = 0.0;
                    // SAFETY: OpenFX `Double2D` parameter writes two `double` out-arguments.
                    if unsafe {
                        get_val_at_time(
                            handle,
                            time,
                            &mut x as *mut c_double,
                            &mut y as *mut c_double,
                        )
                    } == K_OFX_STAT_OK
                    {
                        set.insert(p.name.clone(), ParamValue::Point2D([x, y]));
                    }
                }
            }
        } else if !param_props.is_null() {
            // Non-variadic fallback used by pure-Rust test hosts: reads active values stored on
            // `OfxParamPropDefault` (updated by the test harness per evaluation).
            match &p.kind {
                ParamKind::Double { .. } => {
                    if let Some(v) = get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 0) {
                        set.insert(p.name.clone(), ParamValue::Double(v));
                    }
                }
                ParamKind::Int { .. } => {
                    if let Some(v) = get_int(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 0) {
                        set.insert(p.name.clone(), ParamValue::Int(v));
                    }
                }
                ParamKind::Bool { .. } => {
                    if let Some(v) = get_int(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 0) {
                        set.insert(p.name.clone(), ParamValue::Bool(v != 0));
                    }
                }
                ParamKind::Choice { .. } => {
                    if let Some(v) = get_int(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 0)
                        && let Ok(idx) = usize::try_from(v)
                    {
                        set.insert(p.name.clone(), ParamValue::Choice(idx));
                    }
                }
                ParamKind::Rgba { .. } => {
                    if let (Some(r), Some(g), Some(b), Some(a)) = (
                        get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 0),
                        get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 1),
                        get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 2),
                        get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 3),
                    ) {
                        set.insert(p.name.clone(), ParamValue::Rgba([r, g, b, a]));
                    }
                }
                ParamKind::Point2D { .. } => {
                    if let (Some(x), Some(y)) = (
                        get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 0),
                        get_double(prop, param_props, K_OFX_PARAM_PROP_DEFAULT, 1),
                    ) {
                        set.insert(p.name.clone(), ParamValue::Point2D([x, y]));
                    }
                }
            }
        }
    }

    set
}

fn get_effect_props(
    ie: &OfxImageEffectSuiteV1,
    effect: OfxImageEffectHandle,
) -> Option<OfxPropertySetHandle> {
    let f = ie.get_property_set?;
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    // SAFETY: Calls host `getPropertySet` with valid effect handle.
    if unsafe { f(effect, &mut props) } == K_OFX_STAT_OK && !props.is_null() {
        Some(props)
    } else {
        None
    }
}

fn get_param_set(
    ie: &OfxImageEffectSuiteV1,
    effect: OfxImageEffectHandle,
) -> Option<OfxParamSetHandle> {
    let f = ie.get_param_set?;
    let mut set: OfxParamSetHandle = ptr::null_mut();
    // SAFETY: Calls host `getParamSet` with valid effect handle.
    if unsafe { f(effect, &mut set) } == K_OFX_STAT_OK && !set.is_null() {
        Some(set)
    } else {
        None
    }
}

fn set_cstr(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
    val: &CStr,
) {
    if let Some(f) = prop.prop_set_string {
        // SAFETY: Valid property handle and NUL-terminated C strings.
        let _ = unsafe { f(set, key.as_ptr(), index, val.as_ptr()) };
    }
}

fn set_str(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
    val: &str,
) {
    if let Ok(c) = CString::new(val) {
        set_cstr(prop, set, key, index, &c);
    }
}

fn set_double(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
    val: f64,
) {
    if let Some(f) = prop.prop_set_double {
        // SAFETY: Valid property handle and NUL-terminated key.
        let _ = unsafe { f(set, key.as_ptr(), index, val) };
    }
}

fn set_int(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
    val: c_int,
) {
    if let Some(f) = prop.prop_set_int {
        // SAFETY: Valid property handle and NUL-terminated key.
        let _ = unsafe { f(set, key.as_ptr(), index, val) };
    }
}

fn set_ptr(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
    val: *mut c_void,
) {
    if let Some(f) = prop.prop_set_pointer {
        // SAFETY: Valid property handle and NUL-terminated key.
        let _ = unsafe { f(set, key.as_ptr(), index, val) };
    }
}

fn set_rect_d(prop: &OfxPropertySuiteV1, set: OfxPropertySetHandle, key: &CStr, rect: RectD) {
    set_double(prop, set, key, 0, rect.x1);
    set_double(prop, set, key, 1, rect.y1);
    set_double(prop, set, key, 2, rect.x2);
    set_double(prop, set, key, 3, rect.y2);
}

fn get_double(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
) -> Option<f64> {
    if set.is_null() {
        return None;
    }
    let f = prop.prop_get_double?;
    let mut val: c_double = 0.0;
    // SAFETY: Valid property handle and out-pointer.
    if unsafe { f(set, key.as_ptr(), index, &mut val) } == K_OFX_STAT_OK {
        Some(val)
    } else {
        None
    }
}

fn get_int(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
) -> Option<c_int> {
    if set.is_null() {
        return None;
    }
    let f = prop.prop_get_int?;
    let mut val: c_int = 0;
    // SAFETY: Valid property handle and out-pointer.
    if unsafe { f(set, key.as_ptr(), index, &mut val) } == K_OFX_STAT_OK {
        Some(val)
    } else {
        None
    }
}

fn get_ptr(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
) -> Option<*mut c_void> {
    if set.is_null() {
        return None;
    }
    let f = prop.prop_get_pointer?;
    let mut val: *mut c_void = ptr::null_mut();
    // SAFETY: Valid property handle and out-pointer.
    if unsafe { f(set, key.as_ptr(), index, &mut val) } == K_OFX_STAT_OK {
        Some(val)
    } else {
        None
    }
}

fn get_string(
    prop: &OfxPropertySuiteV1,
    set: OfxPropertySetHandle,
    key: &CStr,
    index: c_int,
) -> Option<String> {
    if set.is_null() {
        return None;
    }
    let f = prop.prop_get_string?;
    let mut ptr_str: *const c_char = ptr::null();
    // SAFETY: Valid property handle and out-pointer for host-owned NUL-terminated string.
    if unsafe { f(set, key.as_ptr(), index, &mut ptr_str) } == K_OFX_STAT_OK && !ptr_str.is_null() {
        // SAFETY: Host guarantees returned string pointer is valid NUL-terminated C string.
        unsafe { CStr::from_ptr(ptr_str) }
            .to_str()
            .ok()
            .map(str::to_owned)
    } else {
        None
    }
}

fn get_rect_d(prop: &OfxPropertySuiteV1, set: OfxPropertySetHandle, key: &CStr) -> Option<RectD> {
    let x1 = get_double(prop, set, key, 0)?;
    let y1 = get_double(prop, set, key, 1)?;
    let x2 = get_double(prop, set, key, 2)?;
    let y2 = get_double(prop, set, key, 3)?;
    Some(RectD::new(x1, y1, x2, y2))
}
