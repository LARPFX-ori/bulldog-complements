//! OpenFX 1.4 C ABI boundary, safe Rust lifecycle dispatcher, plugin descriptor
//! validator, export macros, and in-memory mock OFX host for **Bulldog Complements**.
//!
//! All `unsafe` FFI code is strictly isolated inside [`abi`], [`dispatch`], [`export`],
//! and [`mock_host`] with documented `// SAFETY:` invariants and `catch_unwind` guards
//! on every `extern "C"` entrypoint (AGENTS.md §0.3, §0.7).

#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod abi;
pub mod dispatch;
pub mod export;
pub mod mock_host;
pub mod validate;

pub use abi::{
    K_OFX_STAT_ERR_BAD_HANDLE, K_OFX_STAT_ERR_BAD_INDEX, K_OFX_STAT_ERR_MISSING_HOST_FEATURE,
    K_OFX_STAT_ERR_UNSUPPORTED, K_OFX_STAT_FAILED, K_OFX_STAT_OK, K_OFX_STAT_REPLY_DEFAULT,
    OfxHost, OfxPlugin, OfxPropertySetHandle, OfxStatus,
};
pub use dispatch::PluginSlot;
pub use validate::{REQUIRED_ID_PREFIX, ValidationError, validate_plugin, validate_suite};
