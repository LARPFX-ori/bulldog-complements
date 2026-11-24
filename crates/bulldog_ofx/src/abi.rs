//! Clean-room Rust C ABI definitions conforming to the Academy Software Foundation
//! OpenFX 1.4 open specification.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_char, c_double, c_int, c_uint, c_void};

/// Standard OpenFX status code type (`int`).
pub type OfxStatus = c_int;

/// Timeline coordinate (`double`).
pub type OfxTime = c_double;

/// Opaque OpenFX handles.
pub type OfxPropertySetHandle = *mut c_void;
pub type OfxImageEffectHandle = *mut c_void;
pub type OfxImageClipHandle = *mut c_void;
pub type OfxParamSetHandle = *mut c_void;
pub type OfxParamHandle = *mut c_void;
pub type OfxImageMemoryHandle = *mut c_void;

// --- OpenFX Status Codes ---
pub const K_OFX_STAT_OK: OfxStatus = 0;
pub const K_OFX_STAT_FAILED: OfxStatus = 1;
pub const K_OFX_STAT_ERR_FATAL: OfxStatus = 2;
pub const K_OFX_STAT_ERR_UNKNOWN: OfxStatus = 3;
pub const K_OFX_STAT_ERR_MISSING_HOST_FEATURE: OfxStatus = 4;
pub const K_OFX_STAT_ERR_UNSUPPORTED: OfxStatus = 5;
pub const K_OFX_STAT_ERR_EXISTS: OfxStatus = 6;
pub const K_OFX_STAT_ERR_FORMAT: OfxStatus = 7;
pub const K_OFX_STAT_ERR_MEMORY: OfxStatus = 8;
pub const K_OFX_STAT_ERR_BAD_HANDLE: OfxStatus = 9;
pub const K_OFX_STAT_ERR_BAD_INDEX: OfxStatus = 10;
pub const K_OFX_STAT_ERR_VALUE: OfxStatus = 11;
pub const K_OFX_STAT_REPLY_YES: OfxStatus = 12;
pub const K_OFX_STAT_REPLY_NO: OfxStatus = 13;
pub const K_OFX_STAT_REPLY_DEFAULT: OfxStatus = 14;

// --- API & Suite Identifiers ---
pub const K_OFX_IMAGE_EFFECT_PLUGIN_API: &CStr = c"OfxImageEffectPlugin";
pub const K_OFX_IMAGE_EFFECT_PLUGIN_API_VERSION: c_int = 1;

pub const K_OFX_PROPERTY_SUITE: &CStr = c"OfxPropertySuite";
pub const K_OFX_IMAGE_EFFECT_SUITE: &CStr = c"OfxImageEffectSuite";
pub const K_OFX_PARAMETER_SUITE: &CStr = c"OfxParameterSuite";

// --- Lifecycle Actions ---
pub const K_OFX_ACTION_LOAD: &str = "OfxActionLoad";
pub const K_OFX_ACTION_DESCRIBE: &str = "OfxActionDescribe";
pub const K_OFX_ACTION_UNLOAD: &str = "OfxActionUnload";
pub const K_OFX_ACTION_CREATE_INSTANCE: &str = "OfxActionCreateInstance";
pub const K_OFX_ACTION_DESTROY_INSTANCE: &str = "OfxActionDestroyInstance";
pub const K_OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT: &str =
    "OfxImageEffectActionDescribeInContext";
pub const K_OFX_IMAGE_EFFECT_ACTION_GET_ROD: &str = "OfxImageEffectActionGetRegionOfDefinition";
pub const K_OFX_IMAGE_EFFECT_ACTION_GET_ROI: &str = "OfxImageEffectActionGetRegionsOfInterest";
pub const K_OFX_IMAGE_EFFECT_ACTION_IS_IDENTITY: &str = "OfxImageEffectActionIsIdentity";
pub const K_OFX_IMAGE_EFFECT_ACTION_RENDER: &str = "OfxImageEffectActionRender";

// --- Context Identifiers ---
pub const K_OFX_IMAGE_EFFECT_CONTEXT_FILTER: &CStr = c"OfxImageEffectContextFilter";
pub const K_OFX_IMAGE_EFFECT_CONTEXT_TRANSITION: &CStr = c"OfxImageEffectContextTransition";
pub const K_OFX_IMAGE_EFFECT_CONTEXT_GENERATOR: &CStr = c"OfxImageEffectContextGenerator";
pub const K_OFX_IMAGE_EFFECT_CONTEXT_GENERAL: &CStr = c"OfxImageEffectContextGeneral";

// --- Clip Names ---
pub const K_OFX_SIMPLE_SOURCE_CLIP_NAME: &CStr = c"Source";
pub const K_OFX_TRANSITION_SOURCE_FROM_CLIP_NAME: &CStr = c"SourceFrom";
pub const K_OFX_TRANSITION_SOURCE_TO_CLIP_NAME: &CStr = c"SourceTo";
pub const K_OFX_OUTPUT_CLIP_NAME: &CStr = c"Output";

// --- Bit Depths & Components ---
pub const K_OFX_BIT_DEPTH_BYTE: &CStr = c"OfxBitDepthByte";
pub const K_OFX_BIT_DEPTH_SHORT: &CStr = c"OfxBitDepthShort";
pub const K_OFX_BIT_DEPTH_FLOAT: &CStr = c"OfxBitDepthFloat";
pub const K_OFX_IMAGE_COMPONENT_RGBA: &CStr = c"OfxImageComponentRGBA";

// --- Premultiplication ---
pub const K_OFX_IMAGE_OPAQUE: &CStr = c"OfxImageOpaque";
pub const K_OFX_IMAGE_PREMULTIPLIED: &CStr = c"OfxImagePreMultiplied";
pub const K_OFX_IMAGE_UNPREMULTIPLIED: &CStr = c"OfxImageUnPreMultiplied";

// --- Parameter Types ---
pub const K_OFX_PARAM_TYPE_DOUBLE: &CStr = c"OfxParamTypeDouble";
pub const K_OFX_PARAM_TYPE_INTEGER: &CStr = c"OfxParamTypeInteger";
pub const K_OFX_PARAM_TYPE_BOOLEAN: &CStr = c"OfxParamTypeBoolean";
pub const K_OFX_PARAM_TYPE_CHOICE: &CStr = c"OfxParamTypeChoice";
pub const K_OFX_PARAM_TYPE_RGBA: &CStr = c"OfxParamTypeRGBA";
pub const K_OFX_PARAM_TYPE_DOUBLE2D: &CStr = c"OfxParamTypeDouble2D";

// --- Property Names ---
pub const K_OFX_PROP_LABEL: &CStr = c"OfxPropLabel";
pub const K_OFX_PROP_SHORT_LABEL: &CStr = c"OfxPropShortLabel";
pub const K_OFX_PROP_LONG_LABEL: &CStr = c"OfxPropLongLabel";
pub const K_OFX_PROP_NAME: &CStr = c"OfxPropName";
pub const K_OFX_PROP_TIME: &CStr = c"OfxPropTime";
pub const K_OFX_PROP_INSTANCE_DATA: &CStr = c"OfxPropInstanceData";

pub const K_OFX_IMAGE_EFFECT_PROP_GROUPING: &CStr = c"OfxImageEffectPluginPropGrouping";
pub const K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_CONTEXTS: &CStr =
    c"OfxImageEffectPropSupportedContexts";
pub const K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS: &CStr =
    c"OfxImageEffectPropSupportedPixelDepths";
pub const K_OFX_IMAGE_EFFECT_PROP_SUPPORTED_COMPONENTS: &CStr =
    c"OfxImageEffectPropSupportedComponents";
pub const K_OFX_IMAGE_EFFECT_PROP_SUPPORTS_TILES: &CStr = c"OfxImageEffectPropSupportsTiles";
pub const K_OFX_IMAGE_EFFECT_PROP_CONTEXT: &CStr = c"OfxImageEffectPropContext";
pub const K_OFX_IMAGE_EFFECT_PROP_RENDER_WINDOW: &CStr = c"OfxImageEffectPropRenderWindow";
pub const K_OFX_IMAGE_EFFECT_PROP_RENDER_SCALE: &CStr = c"OfxImageEffectPropRenderScale";
pub const K_OFX_IMAGE_EFFECT_PROP_REGION_OF_DEFINITION: &CStr =
    c"OfxImageEffectPropRegionOfDefinition";
pub const K_OFX_IMAGE_EFFECT_PROP_REGION_OF_INTEREST: &CStr = c"OfxImageEffectPropRegionOfInterest";
pub const K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE: &CStr = c"OfxImageEffectPropRegionOfInterest_Source";
pub const K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_FROM: &CStr =
    c"OfxImageEffectPropRegionOfInterest_SourceFrom";
pub const K_OFX_IMAGE_EFFECT_PROP_ROI_SOURCE_TO: &CStr =
    c"OfxImageEffectPropRegionOfInterest_SourceTo";
pub const K_OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH: &CStr = c"OfxImageEffectPropPixelDepth";
pub const K_OFX_IMAGE_EFFECT_PROP_PREMULTIPLICATION: &CStr = c"OfxImageEffectPropPreMultiplication";
pub const K_OFX_IMAGE_EFFECT_PROP_COMPONENTS: &CStr = c"OfxImageEffectPropComponents";

pub const K_OFX_IMAGE_PROP_DATA: &CStr = c"OfxImagePropData";
pub const K_OFX_IMAGE_PROP_BOUNDS: &CStr = c"OfxImagePropBounds";
pub const K_OFX_IMAGE_PROP_ROW_BYTES: &CStr = c"OfxImagePropRowBytes";

pub const K_OFX_PARAM_PROP_DEFAULT: &CStr = c"OfxParamPropDefault";
pub const K_OFX_PARAM_PROP_MIN: &CStr = c"OfxParamPropMin";
pub const K_OFX_PARAM_PROP_MAX: &CStr = c"OfxParamPropMax";
pub const K_OFX_PARAM_PROP_DISPLAY_MIN: &CStr = c"OfxParamPropDisplayMin";
pub const K_OFX_PARAM_PROP_DISPLAY_MAX: &CStr = c"OfxParamPropDisplayMax";
pub const K_OFX_PARAM_PROP_HINT: &CStr = c"OfxParamPropHint";
pub const K_OFX_PARAM_PROP_CHOICE_OPTION: &CStr = c"OfxParamPropChoiceOption";

/// OpenFX `OfxRectI`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OfxRectI {
    pub x1: c_int,
    pub y1: c_int,
    pub x2: c_int,
    pub y2: c_int,
}

/// OpenFX `OfxRectD`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OfxRectD {
    pub x1: c_double,
    pub y1: c_double,
    pub x2: c_double,
    pub y2: c_double,
}

/// OpenFX plugin main entry function signature.
pub type OfxPluginEntryPoint = unsafe extern "C" fn(
    action: *const c_char,
    handle: *const c_void,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxStatus;

/// OpenFX `OfxHost` struct supplied by the NLE host during `setHost`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OfxHost {
    pub host: OfxPropertySetHandle,
    pub fetch_suite: Option<
        unsafe extern "C" fn(
            host: OfxPropertySetHandle,
            suite_name: *const c_char,
            suite_version: c_int,
        ) -> *const c_void,
    >,
}

/// OpenFX `OfxPlugin` descriptor returned by `OfxGetPlugin`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OfxPlugin {
    pub plugin_api: *const c_char,
    pub api_version: c_int,
    pub plugin_identifier: *const c_char,
    pub plugin_version_major: c_uint,
    pub plugin_version_minor: c_uint,
    pub set_host: Option<unsafe extern "C" fn(host: *mut OfxHost)>,
    pub main_entry: Option<OfxPluginEntryPoint>,
}

// SAFETY: `OfxPlugin` contains only `'static` C strings and immutable function pointers
// once constructed in a static plugin table.
unsafe impl Sync for OfxPlugin {}
// SAFETY: `OfxPlugin` is plain C POD with `'static` pointers.
unsafe impl Send for OfxPlugin {}

/// OpenFX `OfxPropertySuiteV1` function table.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OfxPropertySuiteV1 {
    pub prop_set_pointer: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: *mut c_void,
        ) -> OfxStatus,
    >,
    pub prop_set_string: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: *const c_char,
        ) -> OfxStatus,
    >,
    pub prop_set_double: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: c_double,
        ) -> OfxStatus,
    >,
    pub prop_set_int: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: c_int,
        ) -> OfxStatus,
    >,
    pub prop_set_pointer_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *const *mut c_void,
        ) -> OfxStatus,
    >,
    pub prop_set_string_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *const *const c_char,
        ) -> OfxStatus,
    >,
    pub prop_set_double_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *const c_double,
        ) -> OfxStatus,
    >,
    pub prop_set_int_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *const c_int,
        ) -> OfxStatus,
    >,
    pub prop_get_pointer: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: *mut *mut c_void,
        ) -> OfxStatus,
    >,
    pub prop_get_string: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: *mut *const c_char,
        ) -> OfxStatus,
    >,
    pub prop_get_double: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: *mut c_double,
        ) -> OfxStatus,
    >,
    pub prop_get_int: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            index: c_int,
            value: *mut c_int,
        ) -> OfxStatus,
    >,
    pub prop_get_pointer_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *mut *mut c_void,
        ) -> OfxStatus,
    >,
    pub prop_get_string_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *mut *const c_char,
        ) -> OfxStatus,
    >,
    pub prop_get_double_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *mut c_double,
        ) -> OfxStatus,
    >,
    pub prop_get_int_n: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: c_int,
            values: *mut c_int,
        ) -> OfxStatus,
    >,
    pub prop_reset: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
        ) -> OfxStatus,
    >,
    pub prop_get_dimension: Option<
        unsafe extern "C" fn(
            properties: OfxPropertySetHandle,
            property: *const c_char,
            count: *mut c_int,
        ) -> OfxStatus,
    >,
}

/// OpenFX `OfxImageEffectSuiteV1` function table.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OfxImageEffectSuiteV1 {
    pub get_property_set: Option<
        unsafe extern "C" fn(
            image_effect: OfxImageEffectHandle,
            prop_handle: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub get_param_set: Option<
        unsafe extern "C" fn(
            image_effect: OfxImageEffectHandle,
            param_set: *mut OfxParamSetHandle,
        ) -> OfxStatus,
    >,
    pub clip_define: Option<
        unsafe extern "C" fn(
            image_effect: OfxImageEffectHandle,
            name: *const c_char,
            property_set: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub clip_get_handle: Option<
        unsafe extern "C" fn(
            image_effect: OfxImageEffectHandle,
            name: *const c_char,
            clip: *mut OfxImageClipHandle,
            property_set: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub clip_get_property_set: Option<
        unsafe extern "C" fn(
            clip: OfxImageClipHandle,
            prop_handle: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub clip_get_image: Option<
        unsafe extern "C" fn(
            clip: OfxImageClipHandle,
            time: OfxTime,
            region: *const OfxRectD,
            image_handle: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub clip_release_image:
        Option<unsafe extern "C" fn(image_handle: OfxPropertySetHandle) -> OfxStatus>,
    pub clip_get_region_of_definition: Option<
        unsafe extern "C" fn(
            clip: OfxImageClipHandle,
            time: OfxTime,
            bounds: *mut OfxRectD,
        ) -> OfxStatus,
    >,
    pub abort: Option<unsafe extern "C" fn(image_effect: OfxImageEffectHandle) -> c_int>,
    pub image_memory_alloc: Option<
        unsafe extern "C" fn(
            instance_handle: OfxImageEffectHandle,
            n_bytes: usize,
            memory_handle: *mut OfxImageMemoryHandle,
        ) -> OfxStatus,
    >,
    pub image_memory_free:
        Option<unsafe extern "C" fn(memory_handle: OfxImageMemoryHandle) -> OfxStatus>,
    pub image_memory_lock: Option<
        unsafe extern "C" fn(
            handle: OfxImageMemoryHandle,
            returned_ptr: *mut *mut c_void,
        ) -> OfxStatus,
    >,
    pub image_memory_unlock:
        Option<unsafe extern "C" fn(handle: OfxImageMemoryHandle) -> OfxStatus>,
}

/// OpenFX `OfxParameterSuiteV1` function table.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OfxParameterSuiteV1 {
    pub param_define: Option<
        unsafe extern "C" fn(
            param_set: OfxParamSetHandle,
            param_type: *const c_char,
            name: *const c_char,
            property_set: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub param_get_handle: Option<
        unsafe extern "C" fn(
            param_set: OfxParamSetHandle,
            name: *const c_char,
            param: *mut OfxParamHandle,
            property_set: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub param_set_get_property_set: Option<
        unsafe extern "C" fn(
            param_set: OfxParamSetHandle,
            prop_handle: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub param_get_property_set: Option<
        unsafe extern "C" fn(
            param: OfxParamHandle,
            prop_handle: *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub param_get_value:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle, ...) -> OfxStatus>,
    pub param_get_value_at_time:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, ...) -> OfxStatus>,
    pub param_get_derivative:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, ...) -> OfxStatus>,
    pub param_get_integral: Option<
        unsafe extern "C" fn(
            param_handle: OfxParamHandle,
            time1: OfxTime,
            time2: OfxTime,
            ...
        ) -> OfxStatus,
    >,
    pub param_set_value:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle, ...) -> OfxStatus>,
    pub param_set_value_at_time:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, ...) -> OfxStatus>,
    pub param_get_num_keys: Option<
        unsafe extern "C" fn(
            param_handle: OfxParamHandle,
            number_of_keys: *mut c_uint,
        ) -> OfxStatus,
    >,
    pub param_get_key_time: Option<
        unsafe extern "C" fn(
            param_handle: OfxParamHandle,
            nth_key: c_uint,
            time: *mut OfxTime,
        ) -> OfxStatus,
    >,
    pub param_get_key_index: Option<
        unsafe extern "C" fn(
            param_handle: OfxParamHandle,
            time: OfxTime,
            direction: c_int,
            index: *mut c_int,
        ) -> OfxStatus,
    >,
    pub param_delete_key:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime) -> OfxStatus>,
    pub param_delete_all_keys:
        Option<unsafe extern "C" fn(param_handle: OfxParamHandle) -> OfxStatus>,
    pub param_copy: Option<
        unsafe extern "C" fn(
            param_to: OfxParamHandle,
            param_from: OfxParamHandle,
            dst_offset: OfxTime,
            frame_range: *const OfxRectD,
        ) -> OfxStatus,
    >,
    pub param_edit_begin: Option<
        unsafe extern "C" fn(param_set: OfxParamSetHandle, name: *const c_char) -> OfxStatus,
    >,
    pub param_edit_end: Option<unsafe extern "C" fn(param_set: OfxParamSetHandle) -> OfxStatus>,
}
