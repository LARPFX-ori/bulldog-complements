#![deny(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

bulldog_ofx::export_ofx_cdylib!(transitions_3d::ofx_plugin_count, transitions_3d::ofx_plugin);
