//! Browser clipboard backend for tur, backed by `navigator.clipboard`.
//!
//! Re-exports the clipboard plugin surface from [`tur_engine`] so browser
//! embedders only need this one crate. The backend
//! ([`WasmClipboard`]) is registered via
//! `TurEngineBuilder::capability(Clipboard::new(WasmClipboard))`.

mod backend;

pub use backend::WasmClipboard;
pub use tur_engine::{Clipboard, ClipboardBackend, TurClipboardPlugin};
