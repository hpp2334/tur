//! Native clipboard backend for tur, backed by the [`arboard`] crate
//! (cross-platform: win/mac/linux).
//!
//! Re-exports the clipboard plugin surface from [`tur_engine`] so native
//! embedders only need this one crate. The backend
//! ([`NativeClipboard`]) is registered via
//! `TurEngineBuilder::capability(Clipboard::new(NativeClipboard::new()?))`.
//!
//! On wasm this crate compiles as a near-empty stub (the `arboard` dep is
//! target-gated to `cfg(not(target_family = "wasm"))`). Embedders targeting
//! wasm should depend on `tur-clipboard-wasm` instead.

pub use tur_engine::{Clipboard, ClipboardBackend, TurClipboardPlugin};

#[cfg(not(target_family = "wasm"))]
mod backend;

#[cfg(not(target_family = "wasm"))]
pub use backend::NativeClipboard;
