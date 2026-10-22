//! Browser clipboard backend for tur, backed by `navigator.clipboard`.
//!
//! Re-exports the clipboard plugin surface from [`tur_engine`] so browser
//! embedders only need this one crate. The backend
//! ([`WasmClipboard`]) is registered via
//! `TurRuntimeBuilder::capability(Clipboard::new(WasmClipboard))`.
//!
//! The engine's pump runs on a Web Worker where `navigator.clipboard` does
//! not exist, so every op hops to the main thread over the tagged-message
//! bridge (`bridge`): tur-wasm installs the serving listener on each spawned
//! worker (`serve_host_message`) and routes the replies in the worker's
//! `onmessage` dispatch (`route_worker_message`).

mod backend;
mod bridge;

pub use backend::WasmClipboard;
pub use bridge::{route_worker_message, serve_host_message};
pub use tur_engine::{Clipboard, ClipboardBackend, TurClipboardPlugin};
