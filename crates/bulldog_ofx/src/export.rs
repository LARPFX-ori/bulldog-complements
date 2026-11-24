//! Declarative macros separating collision-free suite registration (`register_ofx_suite!`)
//! from `cdylib` OpenFX C ABI symbol export (`export_ofx_cdylib!`).

/// Registers one or more [`bulldog_core::EffectPlugin`] implementations in a suite `rlib`
/// without emitting unmangled `#[no_mangle]` symbols, allowing multiple suites to be linked
/// together cleanly in tests, benchmarks, and catalog validators.
///
/// Exposes `ofx_plugin_count()` and `ofx_plugin(nth)` for both tests and `export_ofx_cdylib!`.
#[macro_export]
macro_rules! register_ofx_suite {
    (
        $(
            ($slot:ident, $id_cstr:literal, $major:expr, $minor:expr, $factory:expr)
        ),+ $(,)?
    ) => {
        #[allow(unsafe_code)]
        mod bulldog_ofx_suite_registry {
            use super::*;
            use core::ffi::c_int;
            use std::panic::catch_unwind;
            use $crate::abi::{
                K_OFX_IMAGE_EFFECT_PLUGIN_API, K_OFX_IMAGE_EFFECT_PLUGIN_API_VERSION, OfxPlugin,
            };
            use $crate::dispatch::PluginSlot;

            mod slots {
                use super::*;
                $(
                    pub static $slot: PluginSlot = PluginSlot::new($factory);
                )+
            }

            mod main_entry_trampolines {
                use super::slots;
                use core::ffi::{c_char, c_void};
                use std::panic::{AssertUnwindSafe, catch_unwind};
                use $crate::abi::{K_OFX_STAT_FAILED, OfxPropertySetHandle, OfxStatus};

                $(
                    #[allow(non_snake_case)]
                    pub unsafe extern "C" fn $slot(
                        action: *const c_char,
                        handle: *const c_void,
                        in_args: OfxPropertySetHandle,
                        out_args: OfxPropertySetHandle,
                    ) -> OfxStatus {
                        catch_unwind(AssertUnwindSafe(|| {
                            // SAFETY: Forwarded directly from OpenFX host `mainEntry`.
                            unsafe {
                                slots::$slot.handle_main_entry(action, handle, in_args, out_args)
                            }
                        }))
                        .unwrap_or(K_OFX_STAT_FAILED)
                    }
                )+
            }

            mod set_host_trampolines {
                use super::slots;
                use std::panic::{AssertUnwindSafe, catch_unwind};
                use $crate::abi::OfxHost;

                $(
                    #[allow(non_snake_case)]
                    pub unsafe extern "C" fn $slot(host: *mut OfxHost) {
                        let _ = catch_unwind(AssertUnwindSafe(|| {
                            // SAFETY: Forwarded directly from OpenFX host `setHost`.
                            unsafe { slots::$slot.handle_set_host(host) };
                        }));
                    }
                )+
            }

            static PLUGINS: &[OfxPlugin] = &[
                $(
                    OfxPlugin {
                        plugin_api: K_OFX_IMAGE_EFFECT_PLUGIN_API.as_ptr(),
                        api_version: K_OFX_IMAGE_EFFECT_PLUGIN_API_VERSION,
                        plugin_identifier: $id_cstr.as_ptr(),
                        plugin_version_major: $major,
                        plugin_version_minor: $minor,
                        set_host: Some(set_host_trampolines::$slot),
                        main_entry: Some(main_entry_trampolines::$slot),
                    },
                )+
            ];

            /// Returns the number of OpenFX plugins registered in this suite.
            #[must_use]
            pub fn ofx_plugin_count() -> c_int {
                catch_unwind(|| c_int::try_from(PLUGINS.len()).unwrap_or(0)).unwrap_or(0)
            }

            /// Returns a pointer to the `nth` OpenFX plugin descriptor, or NULL if out of bounds.
            #[must_use]
            pub fn ofx_plugin(nth: c_int) -> *const OfxPlugin {
                catch_unwind(|| {
                    let Ok(idx) = usize::try_from(nth) else {
                        return core::ptr::null();
                    };
                    match PLUGINS.get(idx) {
                        Some(p) => core::ptr::from_ref(p),
                        None => core::ptr::null(),
                    }
                })
                .unwrap_or(core::ptr::null())
            }
        }

        pub use bulldog_ofx_suite_registry::{ofx_plugin, ofx_plugin_count};
    };
}

/// Exports the standard unmangled OpenFX C ABI entrypoints (`OfxGetNumberOfPlugins` and
/// `OfxGetPlugin`) from a dedicated `cdylib` bundle crate.
#[macro_export]
macro_rules! export_ofx_cdylib {
    ($count_fn:path, $get_fn:path) => {
        #[allow(unsafe_code)]
        mod bulldog_ofx_cdylib_exports {
            use core::ffi::c_int;
            use std::panic::catch_unwind;
            use $crate::abi::OfxPlugin;

            /// OpenFX required entrypoint returning the number of plugins in this binary.
            #[unsafe(no_mangle)]
            pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
                catch_unwind($count_fn).unwrap_or(0)
            }

            /// OpenFX required entrypoint returning plugin descriptor `nth` or NULL if out of range.
            #[unsafe(no_mangle)]
            pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
                catch_unwind(|| $get_fn(nth)).unwrap_or(core::ptr::null())
            }
        }
    };
}
