//! Native platform integrations for tur.
//!
//! This crate is **native-only** (never compiled for `wasm32` — wasm
//! embedders use `tur-wasm`, which ships its own bundled-font loader and
//! its own Web-Worker-based pooling). Currently provides:
//!
//! - [`NativeFontLoader`] — a [`FontLoader`] that discovers installed
//!   system fonts via fontique's platform backends (CoreText,
//!   DirectWrite, fontconfig).
//! - [`worker_pool`] — the native worker-pool executor
//!   ([`NativeWorkerPools`]) backing
//!   `MainSchedulerDriver::spawn_worker_in` for native embedders.
//!
//! [`FontLoader`]: tur_engine::core::fonts::FontLoader

#[cfg(target_arch = "wasm32")]
compile_error!("tur-native is native-only; wasm embedders use tur-wasm instead");

pub mod fonts;
pub mod worker_pool;

pub use fonts::NativeFontLoader;
