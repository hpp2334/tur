//! Native clipboard backend for tur, backed by the [`arboard`] crate
//! (cross-platform: win/mac/linux).
//!
//! Re-exports the clipboard plugin surface from [`tur_engine`] so native
//! embedders only need this one crate. The backend
//! ([`NativeClipboard`]) is registered via the closure form of
//! `TurRuntimeBuilder::capability`:
//!
//! ```no_run
//! # use tur_clipboard_native::{Clipboard, NativeClipboard};
//! # use tur_engine::TurRuntimeBuilder;
//! TurRuntimeBuilder::new()
//!     .capability(|cx| Ok(Clipboard::new(NativeClipboard::new(cx)?)))
//!     // .plugin(tur_engine::TurClipboardPlugin) ...
//! # ;
//! ```
//!
//! The closure receives an [`AsyncPluginContext`] — each `arboard` call is
//! hopped onto the engine's main thread (macOS `NSPasteboard` requires it).
//! The engine creates the channel internally, so no extra wiring is needed.
//!
//! On wasm this crate compiles as a near-empty stub (the `arboard` dep is
//! target-gated to `cfg(not(target_family = "wasm"))`). Embedders targeting
//! wasm should depend on `tur-clipboard-wasm` instead.
//!
//! [`AsyncPluginContext`]: tur_engine::AsyncPluginContext

pub use tur_engine::{AsyncPluginContext, Clipboard, ClipboardBackend, TurClipboardPlugin};

#[cfg(not(target_family = "wasm"))]
mod backend;

#[cfg(not(target_family = "wasm"))]
pub use backend::NativeClipboard;
