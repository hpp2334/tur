//! Browser clipboard backend for tur, backed by `navigator.clipboard`.
//!
//! Re-exports the full clipboard surface from [`tur_clipboard_capability`]
//! so browser embedders only need this one crate. The backend
//! ([`WasmClipboard`]) is registered via
//! `TurEngineBuilder::capability(Clipboard::new(WasmClipboard))`.

mod backend;

pub use tur_clipboard_capability::{
    Clipboard, ClipboardBackend, ClipboardWriteSubsystem, NoopClipboard,
    TurClipboardPlugin,
};
pub use backend::WasmClipboard;
