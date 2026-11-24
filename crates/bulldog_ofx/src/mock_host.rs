//! In-memory OpenFX 1.4 mock host for exercising real `extern "C"` `OfxPlugin`
//! entrypoints end-to-end inside `cargo test` without requiring an external NLE.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_char, c_double, c_int, c_void};
use core::ptr;
use std::collections::BTreeMap;
use std::ffi::CString;

use bulldog_core::{EffectContext, ParamValue, PixelFormat, PreMultiplication, RectD};

use crate::abi::{
    K_OFX_ACTION_CREATE_INSTANCE, K_OFX_ACTION_DESCRIBE, K_OFX_ACTION_DESTROY_INSTANCE,
    K_OFX_ACTION_LOAD, K_OFX_ACTION_UNLOAD, K_OFX_BIT_DEPTH_BYTE, K_OFX_BIT_DEPTH_FLOAT,
    K_OFX_BIT_DEPTH_SHORT, K_OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT,
    K_OFX_IMAGE_EFFECT_ACTION_GET_ROD, K_OFX_IMAGE_EFFECT_ACTION_GET_ROI,
    K_OFX_IMAGE_EFFECT_ACTION_IS_IDENTITY, K_OFX_IMAGE_EFFECT_ACTION_RENDER,
    K_OFX_IMAGE_EFFECT_CONTEXT_FILTER, K_OFX_IMAGE_EFFECT_CONTEXT_GENERAL,
    K_OFX_IMAGE_EFFECT_CONTEXT_GENERATOR, K_OFX_IMAGE_EFFECT_CONTEXT_TRANSITION,
    K_OFX_IMAGE_EFFECT_PROP_CONTEXT, K_OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH,
    K_OFX_IMAGE_EFFECT_PROP_PREMULTIPLICATION, K_OFX_IMAGE_EFFECT_PROP_REGION_OF_DEFINITION,
    K_OFX_IMAGE_EFFECT_PROP_REGION_OF_INTEREST, K_OFX_IMAGE_EFFECT_PROP_RENDER_SCALE,
    K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE, K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_FROM,
    K_OFX_IMAGE_EFFECT_SUITE, K_OFX_IMAGE_OPAQUE, K_OFX_IMAGE_PREMULTIPLIED,
    K_OFX_IMAGE_PROP_BOUNDS, K_OFX_IMAGE_PROP_DATA, K_OFX_IMAGE_PROP_ROW_BYTES,
    K_OFX_IMAGE_UNPREMULTIPLIED, K_OFX_PARAM_PROP_DEFAULT, K_OFX_PARAMETER_SUITE, K_OFX_PROP_NAME,
    K_OFX_PROP_TIME, K_OFX_PROPERTY_SUITE, K_OFX_STAT_ERR_BAD_HANDLE, K_OFX_STAT_ERR_BAD_INDEX,
    K_OFX_STAT_ERR_UNKNOWN, K_OFX_STAT_FAILED, K_OFX_STAT_OK, OfxHost, OfxImageClipHandle,
    OfxImageEffectHandle, OfxImageEffectSuiteV1, OfxParamHandle, OfxParamSetHandle,
    OfxParameterSuiteV1, OfxPlugin, OfxPropertySetHandle, OfxPropertySuiteV1, OfxRectD, OfxStatus,
    OfxTime,
};

#[derive(Default)]
struct MockPropertySet {
    ints: BTreeMap<(String, c_int), c_int>,
    doubles: BTreeMap<(String, c_int), c_double>,
    strings: BTreeMap<(String, c_int), CString>,
    ptrs: BTreeMap<(String, c_int), *mut c_void>,
}

struct MockParam {
    props: Box<MockPropertySet>,
}

struct MockClipImage {
    data: Vec<u8>,
    width: u32,
    height: u32,
    row_bytes: isize,
    base_offset: usize,
    format: PixelFormat,
    premult: PreMultiplication,
    props: Box<MockPropertySet>,
}

struct MockClip {
    props: Box<MockPropertySet>,
    image: Option<MockClipImage>,
}

struct MockEffectState {
    props: Box<MockPropertySet>,
    params: BTreeMap<String, Box<MockParam>>,
    clips: BTreeMap<String, Box<MockClip>>,
    aborted: bool,
    released_images: usize,
}

/// Configured clip image passed into [`run_mock_lifecycle`].
#[derive(Debug, Clone)]
pub struct MockImageSpec {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub row_bytes: isize,
    pub base_offset: usize,
    pub format: PixelFormat,
    pub premult: PreMultiplication,
}

impl MockImageSpec {
    /// Creates a tightly packed top-down image buffer specification.
    #[must_use]
    pub fn tightly_packed(
        data: Vec<u8>,
        width: u32,
        height: u32,
        format: PixelFormat,
        premult: PreMultiplication,
    ) -> Self {
        let row_bytes = (width as isize) * (format.bytes_per_pixel() as isize);
        Self {
            data,
            width,
            height,
            row_bytes,
            base_offset: 0,
            format,
            premult,
        }
    }
}

/// Request driving a full OpenFX C ABI lifecycle session against an exported [`OfxPlugin`].
#[derive(Debug, Clone)]
pub struct MockLifecycleRequest {
    pub context: EffectContext,
    pub time: f64,
    pub clips: BTreeMap<String, MockImageSpec>,
    pub param_overrides: BTreeMap<String, ParamValue>,
    pub aborted: bool,
}

/// Observed results of running the entire OpenFX C ABI lifecycle on an exported [`OfxPlugin`].
#[derive(Debug)]
pub struct MockLifecycleReport {
    pub load_status: OfxStatus,
    pub describe_status: OfxStatus,
    pub describe_in_context_status: OfxStatus,
    pub create_status: OfxStatus,
    pub rod_status: OfxStatus,
    pub rod: RectD,
    pub roi_status: OfxStatus,
    pub roi: RectD,
    pub is_identity_status: OfxStatus,
    pub is_identity_clip: Option<String>,
    pub render_status: OfxStatus,
    pub destroy_status: OfxStatus,
    pub unload_status: OfxStatus,
    pub output_bytes: Vec<u8>,
    pub released_images: usize,
}

/// Drives a raw exported `*const OfxPlugin` through the full OpenFX 1.4 C ABI lifecycle:
/// `setHost` -> `Load` -> `Describe` -> `DescribeInContext` -> `CreateInstance` ->
/// `GetRoD` -> `GetRoI` -> `IsIdentity` -> `Render` -> `DestroyInstance` -> `Unload`.
///
/// # Safety
/// `plugin_ptr` must be a valid pointer returned by `OfxGetPlugin`.
pub unsafe fn run_mock_lifecycle(
    plugin_ptr: *const OfxPlugin,
    request: MockLifecycleRequest,
) -> Option<MockLifecycleReport> {
    if plugin_ptr.is_null() {
        return None;
    }
    // SAFETY: Caller provides a valid static `OfxPlugin` pointer from `OfxGetPlugin`.
    let plugin = unsafe { *plugin_ptr };
    let set_host = plugin.set_host?;
    let main_entry = plugin.main_entry?;

    let mut host_props = Box::new(MockPropertySet::default());
    let mut ofx_host = OfxHost {
        host: core::ptr::from_mut(host_props.as_mut()).cast::<c_void>(),
        fetch_suite: Some(mock_fetch_suite),
    };
    // SAFETY: `ofx_host` stays pinned on stack throughout this lifecycle call.
    unsafe { set_host(&mut ofx_host) };

    let mut state = Box::new(MockEffectState {
        props: Box::new(MockPropertySet::default()),
        params: BTreeMap::new(),
        clips: BTreeMap::new(),
        aborted: request.aborted,
        released_images: 0,
    });
    let state_ptr = core::ptr::from_mut(state.as_mut()).cast::<c_void>();

    let call = |action: &str,
                handle: *const c_void,
                in_args: OfxPropertySetHandle,
                out_args: OfxPropertySetHandle|
     -> OfxStatus {
        let Ok(c_action) = CString::new(action) else {
            return K_OFX_STAT_FAILED;
        };
        // SAFETY: Valid NUL-terminated action string and host-owned property sets.
        unsafe { main_entry(c_action.as_ptr(), handle, in_args, out_args) }
    };

    let load_status = call(
        K_OFX_ACTION_LOAD,
        ptr::null(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    let describe_status = call(
        K_OFX_ACTION_DESCRIBE,
        state_ptr,
        ptr::null_mut(),
        ptr::null_mut(),
    );

    let mut ctx_in = Box::new(MockPropertySet::default());
    let ctx_cstr = match request.context {
        EffectContext::Filter => K_OFX_IMAGE_EFFECT_CONTEXT_FILTER,
        EffectContext::Transition => K_OFX_IMAGE_EFFECT_CONTEXT_TRANSITION,
        EffectContext::Generator => K_OFX_IMAGE_EFFECT_CONTEXT_GENERATOR,
        EffectContext::General => K_OFX_IMAGE_EFFECT_CONTEXT_GENERAL,
    };
    ctx_in.strings.insert(
        (
            K_OFX_IMAGE_EFFECT_PROP_CONTEXT
                .to_str()
                .unwrap_or("")
                .to_owned(),
            0,
        ),
        ctx_cstr.to_owned(),
    );
    let describe_in_context_status = call(
        K_OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT,
        state_ptr,
        core::ptr::from_mut(ctx_in.as_mut()).cast::<c_void>(),
        ptr::null_mut(),
    );

    // Populate clip images and parameter overrides.
    for (name, spec) in request.clips {
        let mut img_props = Box::new(MockPropertySet::default());
        img_props.ptrs.insert(("__state".to_owned(), 0), state_ptr);
        let clip_entry = state.clips.entry(name).or_insert_with(|| {
            Box::new(MockClip {
                props: Box::new(MockPropertySet::default()),
                image: None,
            })
        });
        clip_entry.image = Some(MockClipImage {
            data: spec.data,
            width: spec.width,
            height: spec.height,
            row_bytes: spec.row_bytes,
            base_offset: spec.base_offset,
            format: spec.format,
            premult: spec.premult,
            props: img_props,
        });
    }

    let def_key = K_OFX_PARAM_PROP_DEFAULT.to_str().unwrap_or("").to_owned();
    for (p_name, p_val) in request.param_overrides {
        if let Some(param) = state.params.get_mut(&p_name) {
            match p_val {
                ParamValue::Double(v) => {
                    param.props.doubles.insert((def_key.clone(), 0), v);
                }
                ParamValue::Int(v) => {
                    param.props.ints.insert((def_key.clone(), 0), v);
                }
                ParamValue::Bool(v) => {
                    param
                        .props
                        .ints
                        .insert((def_key.clone(), 0), c_int::from(v));
                }
                ParamValue::Choice(v) => {
                    param.props.ints.insert((def_key.clone(), 0), v as c_int);
                }
                ParamValue::Rgba([r, g, b, a]) => {
                    param.props.doubles.insert((def_key.clone(), 0), r);
                    param.props.doubles.insert((def_key.clone(), 1), g);
                    param.props.doubles.insert((def_key.clone(), 2), b);
                    param.props.doubles.insert((def_key.clone(), 3), a);
                }
                ParamValue::Point2D([x, y]) => {
                    param.props.doubles.insert((def_key.clone(), 0), x);
                    param.props.doubles.insert((def_key.clone(), 1), y);
                }
            }
        }
    }

    let create_status = call(
        K_OFX_ACTION_CREATE_INSTANCE,
        state_ptr,
        ptr::null_mut(),
        ptr::null_mut(),
    );

    let time_key = K_OFX_PROP_TIME.to_str().unwrap_or("").to_owned();
    let mut time_in = Box::new(MockPropertySet::default());
    time_in.doubles.insert((time_key.clone(), 0), request.time);

    // GetRegionOfDefinition
    let mut rod_out = Box::new(MockPropertySet::default());
    let rod_status = call(
        K_OFX_IMAGE_EFFECT_ACTION_GET_ROD,
        state_ptr,
        core::ptr::from_mut(time_in.as_mut()).cast::<c_void>(),
        core::ptr::from_mut(rod_out.as_mut()).cast::<c_void>(),
    );
    let rod_key = K_OFX_IMAGE_EFFECT_PROP_REGION_OF_DEFINITION
        .to_str()
        .unwrap_or("")
        .to_owned();
    let rod = RectD::new(
        rod_out
            .doubles
            .get(&(rod_key.clone(), 0))
            .copied()
            .unwrap_or(0.0),
        rod_out
            .doubles
            .get(&(rod_key.clone(), 1))
            .copied()
            .unwrap_or(0.0),
        rod_out
            .doubles
            .get(&(rod_key.clone(), 2))
            .copied()
            .unwrap_or(0.0),
        rod_out.doubles.get(&(rod_key, 3)).copied().unwrap_or(0.0),
    );

    // GetRegionsOfInterest
    let mut roi_in = Box::new(MockPropertySet::default());
    roi_in.doubles.insert((time_key.clone(), 0), request.time);
    let roi_in_key = K_OFX_IMAGE_EFFECT_PROP_REGION_OF_INTEREST
        .to_str()
        .unwrap_or("")
        .to_owned();
    roi_in.doubles.insert((roi_in_key.clone(), 0), 0.0);
    roi_in.doubles.insert((roi_in_key.clone(), 1), 0.0);
    roi_in.doubles.insert((roi_in_key.clone(), 2), 64.0);
    roi_in.doubles.insert((roi_in_key, 3), 64.0);
    let mut roi_out = Box::new(MockPropertySet::default());
    let roi_status = call(
        K_OFX_IMAGE_EFFECT_ACTION_GET_ROI,
        state_ptr,
        core::ptr::from_mut(roi_in.as_mut()).cast::<c_void>(),
        core::ptr::from_mut(roi_out.as_mut()).cast::<c_void>(),
    );
    let roi_out_key = if request.context == EffectContext::Transition {
        K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_FROM
    } else {
        K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE
    }
    .to_str()
    .unwrap_or("")
    .to_owned();
    let roi = RectD::new(
        roi_out
            .doubles
            .get(&(roi_out_key.clone(), 0))
            .copied()
            .unwrap_or(0.0),
        roi_out
            .doubles
            .get(&(roi_out_key.clone(), 1))
            .copied()
            .unwrap_or(0.0),
        roi_out
            .doubles
            .get(&(roi_out_key.clone(), 2))
            .copied()
            .unwrap_or(0.0),
        roi_out
            .doubles
            .get(&(roi_out_key, 3))
            .copied()
            .unwrap_or(0.0),
    );

    // IsIdentity
    let mut id_out = Box::new(MockPropertySet::default());
    let is_identity_status = call(
        K_OFX_IMAGE_EFFECT_ACTION_IS_IDENTITY,
        state_ptr,
        core::ptr::from_mut(time_in.as_mut()).cast::<c_void>(),
        core::ptr::from_mut(id_out.as_mut()).cast::<c_void>(),
    );
    let name_key = K_OFX_PROP_NAME.to_str().unwrap_or("").to_owned();
    let is_identity_clip = if is_identity_status == K_OFX_STAT_OK {
        id_out
            .strings
            .get(&(name_key, 0))
            .and_then(|s| s.to_str().ok())
            .map(str::to_owned)
    } else {
        None
    };

    // Render
    let mut render_in = Box::new(MockPropertySet::default());
    render_in.doubles.insert((time_key, 0), request.time);
    let rs_key = K_OFX_IMAGE_EFFECT_PROP_RENDER_SCALE
        .to_str()
        .unwrap_or("")
        .to_owned();
    render_in.doubles.insert((rs_key.clone(), 0), 1.0);
    render_in.doubles.insert((rs_key, 1), 1.0);
    let render_status = call(
        K_OFX_IMAGE_EFFECT_ACTION_RENDER,
        state_ptr,
        core::ptr::from_mut(render_in.as_mut()).cast::<c_void>(),
        ptr::null_mut(),
    );

    let destroy_status = call(
        K_OFX_ACTION_DESTROY_INSTANCE,
        state_ptr,
        ptr::null_mut(),
        ptr::null_mut(),
    );
    let unload_status = call(
        K_OFX_ACTION_UNLOAD,
        ptr::null(),
        ptr::null_mut(),
        ptr::null_mut(),
    );

    let output_bytes = state
        .clips
        .get("Output")
        .and_then(|c| c.image.as_ref())
        .map(|img| img.data.clone())
        .unwrap_or_default();

    Some(MockLifecycleReport {
        load_status,
        describe_status,
        describe_in_context_status,
        create_status,
        rod_status,
        rod,
        roi_status,
        roi,
        is_identity_status,
        is_identity_clip,
        render_status,
        destroy_status,
        unload_status,
        output_bytes,
        released_images: state.released_images,
    })
}

static PROP_SUITE: OfxPropertySuiteV1 = OfxPropertySuiteV1 {
    prop_set_pointer: Some(mock_prop_set_pointer),
    prop_set_string: Some(mock_prop_set_string),
    prop_set_double: Some(mock_prop_set_double),
    prop_set_int: Some(mock_prop_set_int),
    prop_set_pointer_n: None,
    prop_set_string_n: None,
    prop_set_double_n: None,
    prop_set_int_n: None,
    prop_get_pointer: Some(mock_prop_get_pointer),
    prop_get_string: Some(mock_prop_get_string),
    prop_get_double: Some(mock_prop_get_double),
    prop_get_int: Some(mock_prop_get_int),
    prop_get_pointer_n: None,
    prop_get_string_n: None,
    prop_get_double_n: None,
    prop_get_int_n: None,
    prop_reset: None,
    prop_get_dimension: None,
};

static IMAGE_EFFECT_SUITE: OfxImageEffectSuiteV1 = OfxImageEffectSuiteV1 {
    get_property_set: Some(mock_ie_get_property_set),
    get_param_set: Some(mock_ie_get_param_set),
    clip_define: Some(mock_ie_clip_define),
    clip_get_handle: Some(mock_ie_clip_get_handle),
    clip_get_property_set: Some(mock_ie_clip_get_property_set),
    clip_get_image: Some(mock_ie_clip_get_image),
    clip_release_image: Some(mock_ie_clip_release_image),
    clip_get_region_of_definition: Some(mock_ie_clip_get_rod),
    abort: Some(mock_ie_abort),
    image_memory_alloc: None,
    image_memory_free: None,
    image_memory_lock: None,
    image_memory_unlock: None,
};

static PARAM_SUITE: OfxParameterSuiteV1 = OfxParameterSuiteV1 {
    param_define: Some(mock_param_define),
    param_get_handle: Some(mock_param_get_handle),
    param_set_get_property_set: None,
    param_get_property_set: Some(mock_param_get_property_set),
    param_get_value: None,
    // Intentionally `None` in pure-Rust mock host so `dispatch` reads active test values
    // from `OfxParamPropDefault` without non-portable variadic trampolines.
    param_get_value_at_time: None,
    param_get_derivative: None,
    param_get_integral: None,
    param_set_value: None,
    param_set_value_at_time: None,
    param_get_num_keys: None,
    param_get_key_time: None,
    param_get_key_index: None,
    param_delete_key: None,
    param_delete_all_keys: None,
    param_copy: None,
    param_edit_begin: None,
    param_edit_end: None,
};

unsafe extern "C" fn mock_fetch_suite(
    _host: OfxPropertySetHandle,
    suite_name: *const c_char,
    version: c_int,
) -> *const c_void {
    if suite_name.is_null() || version != 1 {
        return ptr::null();
    }
    // SAFETY: Caller passes NUL-terminated suite name.
    let Ok(name) = unsafe { CStr::from_ptr(suite_name) }.to_str() else {
        return ptr::null();
    };
    if name == K_OFX_PROPERTY_SUITE.to_str().unwrap_or("") {
        core::ptr::from_ref(&PROP_SUITE).cast::<c_void>()
    } else if name == K_OFX_IMAGE_EFFECT_SUITE.to_str().unwrap_or("") {
        core::ptr::from_ref(&IMAGE_EFFECT_SUITE).cast::<c_void>()
    } else if name == K_OFX_PARAMETER_SUITE.to_str().unwrap_or("") {
        core::ptr::from_ref(&PARAM_SUITE).cast::<c_void>()
    } else {
        ptr::null()
    }
}

unsafe extern "C" fn mock_prop_set_pointer(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: *mut c_void,
) -> OfxStatus {
    if props.is_null() || key.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet`.
    let ps = unsafe { &mut *props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    ps.ptrs.insert((k.to_owned(), index), val);
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_prop_set_string(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: *const c_char,
) -> OfxStatus {
    if props.is_null() || key.is_null() || val.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet`; `key` and `val` are NUL-terminated.
    let ps = unsafe { &mut *props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    let v = unsafe { CStr::from_ptr(val) }.to_owned();
    ps.strings.insert((k.to_owned(), index), v);
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_prop_set_double(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: c_double,
) -> OfxStatus {
    if props.is_null() || key.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet`.
    let ps = unsafe { &mut *props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    ps.doubles.insert((k.to_owned(), index), val);
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_prop_set_int(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: c_int,
) -> OfxStatus {
    if props.is_null() || key.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet`.
    let ps = unsafe { &mut *props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    ps.ints.insert((k.to_owned(), index), val);
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_prop_get_pointer(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: *mut *mut c_void,
) -> OfxStatus {
    if props.is_null() || key.is_null() || val.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet` and `val` is writable.
    let ps = unsafe { &*props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    match ps.ptrs.get(&(k.to_owned(), index)) {
        Some(&p) => {
            unsafe { *val = p };
            K_OFX_STAT_OK
        }
        None => K_OFX_STAT_ERR_BAD_INDEX,
    }
}

unsafe extern "C" fn mock_prop_get_string(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: *mut *const c_char,
) -> OfxStatus {
    if props.is_null() || key.is_null() || val.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet` and `val` is writable.
    let ps = unsafe { &*props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    match ps.strings.get(&(k.to_owned(), index)) {
        Some(s) => {
            unsafe { *val = s.as_ptr() };
            K_OFX_STAT_OK
        }
        None => K_OFX_STAT_ERR_BAD_INDEX,
    }
}

unsafe extern "C" fn mock_prop_get_double(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: *mut c_double,
) -> OfxStatus {
    if props.is_null() || key.is_null() || val.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet` and `val` is writable.
    let ps = unsafe { &*props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    match ps.doubles.get(&(k.to_owned(), index)) {
        Some(&d) => {
            unsafe { *val = d };
            K_OFX_STAT_OK
        }
        None => K_OFX_STAT_ERR_BAD_INDEX,
    }
}

unsafe extern "C" fn mock_prop_get_int(
    props: OfxPropertySetHandle,
    key: *const c_char,
    index: c_int,
    val: *mut c_int,
) -> OfxStatus {
    if props.is_null() || key.is_null() || val.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `props` points to `MockPropertySet` and `val` is writable.
    let ps = unsafe { &*props.cast::<MockPropertySet>() };
    let Ok(k) = unsafe { CStr::from_ptr(key) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    match ps.ints.get(&(k.to_owned(), index)) {
        Some(&i) => {
            unsafe { *val = i };
            K_OFX_STAT_OK
        }
        None => K_OFX_STAT_ERR_BAD_INDEX,
    }
}

unsafe extern "C" fn mock_ie_get_property_set(
    effect: OfxImageEffectHandle,
    prop_handle: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if effect.is_null() || prop_handle.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `effect` points to `MockEffectState`.
    let state = unsafe { &mut *effect.cast::<MockEffectState>() };
    unsafe {
        *prop_handle = core::ptr::from_mut(state.props.as_mut()).cast::<c_void>();
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_get_param_set(
    effect: OfxImageEffectHandle,
    param_set: *mut OfxParamSetHandle,
) -> OfxStatus {
    if effect.is_null() || param_set.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: Mock host uses `effect` handle directly as its parameter set handle.
    unsafe { *param_set = effect };
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_clip_define(
    effect: OfxImageEffectHandle,
    name: *const c_char,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if effect.is_null() || name.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `effect` points to `MockEffectState`; `name` is NUL-terminated.
    let state = unsafe { &mut *effect.cast::<MockEffectState>() };
    let Ok(clip_name) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    let entry = state.clips.entry(clip_name.to_owned()).or_insert_with(|| {
        Box::new(MockClip {
            props: Box::new(MockPropertySet::default()),
            image: None,
        })
    });
    if !property_set.is_null() {
        unsafe {
            *property_set = core::ptr::from_mut(entry.props.as_mut()).cast::<c_void>();
        }
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_clip_get_handle(
    effect: OfxImageEffectHandle,
    name: *const c_char,
    clip: *mut OfxImageClipHandle,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if effect.is_null() || name.is_null() || clip.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `effect` points to `MockEffectState`.
    let state = unsafe { &mut *effect.cast::<MockEffectState>() };
    let Ok(clip_name) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    let Some(entry) = state.clips.get_mut(clip_name) else {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    };
    unsafe {
        *clip = core::ptr::from_mut(entry.as_mut()).cast::<c_void>();
        if !property_set.is_null() {
            *property_set = core::ptr::from_mut(entry.props.as_mut()).cast::<c_void>();
        }
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_clip_get_property_set(
    clip: OfxImageClipHandle,
    prop_handle: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if clip.is_null() || prop_handle.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `clip` points to `MockClip`.
    let c = unsafe { &mut *clip.cast::<MockClip>() };
    unsafe {
        *prop_handle = core::ptr::from_mut(c.props.as_mut()).cast::<c_void>();
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_clip_get_image(
    clip: OfxImageClipHandle,
    _time: OfxTime,
    _region: *const OfxRectD,
    image_handle: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if clip.is_null() || image_handle.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `clip` points to `MockClip`.
    let c = unsafe { &mut *clip.cast::<MockClip>() };
    let Some(img) = c.image.as_mut() else {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    };
    let data_ptr = if img.data.is_empty() || img.base_offset > img.data.len() {
        ptr::null_mut()
    } else {
        // SAFETY: Checked `img.base_offset <= img.data.len()` above.
        unsafe { img.data.as_mut_ptr().add(img.base_offset) }.cast::<c_void>()
    };
    img.props.ptrs.insert(
        (K_OFX_IMAGE_PROP_DATA.to_str().unwrap_or("").to_owned(), 0),
        data_ptr,
    );
    let bounds_key = K_OFX_IMAGE_PROP_BOUNDS.to_str().unwrap_or("").to_owned();
    img.props.ints.insert((bounds_key.clone(), 0), 0);
    img.props.ints.insert((bounds_key.clone(), 1), 0);
    img.props
        .ints
        .insert((bounds_key.clone(), 2), img.width as c_int);
    img.props.ints.insert((bounds_key, 3), img.height as c_int);
    img.props.ints.insert(
        (
            K_OFX_IMAGE_PROP_ROW_BYTES.to_str().unwrap_or("").to_owned(),
            0,
        ),
        img.row_bytes as c_int,
    );
    let depth = match img.format {
        PixelFormat::Rgba8 => K_OFX_BIT_DEPTH_BYTE,
        PixelFormat::Rgba16 => K_OFX_BIT_DEPTH_SHORT,
        PixelFormat::Rgba32F => K_OFX_BIT_DEPTH_FLOAT,
    };
    img.props.strings.insert(
        (
            K_OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH
                .to_str()
                .unwrap_or("")
                .to_owned(),
            0,
        ),
        depth.to_owned(),
    );
    let pm = match img.premult {
        PreMultiplication::Opaque => K_OFX_IMAGE_OPAQUE,
        PreMultiplication::PreMultiplied => K_OFX_IMAGE_PREMULTIPLIED,
        PreMultiplication::UnPreMultiplied => K_OFX_IMAGE_UNPREMULTIPLIED,
    };
    img.props.strings.insert(
        (
            K_OFX_IMAGE_EFFECT_PROP_PREMULTIPLICATION
                .to_str()
                .unwrap_or("")
                .to_owned(),
            0,
        ),
        pm.to_owned(),
    );
    unsafe {
        *image_handle = core::ptr::from_mut(img.props.as_mut()).cast::<c_void>();
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_clip_release_image(image_handle: OfxPropertySetHandle) -> OfxStatus {
    if image_handle.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `image_handle` points to a `MockPropertySet` storing back-pointer `"__state"`.
    let ps = unsafe { &*image_handle.cast::<MockPropertySet>() };
    if let Some(&state_ptr) = ps.ptrs.get(&("__state".to_owned(), 0))
        && !state_ptr.is_null()
    {
        let state = unsafe { &mut *state_ptr.cast::<MockEffectState>() };
        state.released_images = state.released_images.saturating_add(1);
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_clip_get_rod(
    clip: OfxImageClipHandle,
    _time: OfxTime,
    bounds: *mut OfxRectD,
) -> OfxStatus {
    if clip.is_null() || bounds.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `clip` points to `MockClip`.
    let c = unsafe { &*clip.cast::<MockClip>() };
    let (w, h) = match &c.image {
        Some(img) => (f64::from(img.width), f64::from(img.height)),
        None => (0.0, 0.0),
    };
    unsafe {
        *bounds = OfxRectD {
            x1: 0.0,
            y1: 0.0,
            x2: w,
            y2: h,
        };
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_ie_abort(effect: OfxImageEffectHandle) -> c_int {
    if effect.is_null() {
        return 0;
    }
    // SAFETY: `effect` points to `MockEffectState`.
    let state = unsafe { &*effect.cast::<MockEffectState>() };
    c_int::from(state.aborted)
}

unsafe extern "C" fn mock_param_define(
    param_set: OfxParamSetHandle,
    _param_type: *const c_char,
    name: *const c_char,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if param_set.is_null() || name.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `param_set` points to `MockEffectState`.
    let state = unsafe { &mut *param_set.cast::<MockEffectState>() };
    let Ok(p_name) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    let entry = state.params.entry(p_name.to_owned()).or_insert_with(|| {
        Box::new(MockParam {
            props: Box::new(MockPropertySet::default()),
        })
    });
    if !property_set.is_null() {
        unsafe {
            *property_set = core::ptr::from_mut(entry.props.as_mut()).cast::<c_void>();
        }
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_param_get_handle(
    param_set: OfxParamSetHandle,
    name: *const c_char,
    param: *mut OfxParamHandle,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if param_set.is_null() || name.is_null() || param.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `param_set` points to `MockEffectState`.
    let state = unsafe { &mut *param_set.cast::<MockEffectState>() };
    let Ok(p_name) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return K_OFX_STAT_ERR_UNKNOWN;
    };
    let Some(entry) = state.params.get_mut(p_name) else {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    };
    unsafe {
        *param = core::ptr::from_mut(entry.as_mut()).cast::<c_void>();
        if !property_set.is_null() {
            *property_set = core::ptr::from_mut(entry.props.as_mut()).cast::<c_void>();
        }
    }
    K_OFX_STAT_OK
}

unsafe extern "C" fn mock_param_get_property_set(
    param: OfxParamHandle,
    prop_handle: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if param.is_null() || prop_handle.is_null() {
        return K_OFX_STAT_ERR_BAD_HANDLE;
    }
    // SAFETY: `param` points to `MockParam`.
    let p = unsafe { &mut *param.cast::<MockParam>() };
    unsafe {
        *prop_handle = core::ptr::from_mut(p.props.as_mut()).cast::<c_void>();
    }
    K_OFX_STAT_OK
}
