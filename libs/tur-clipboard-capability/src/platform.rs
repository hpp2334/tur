//! Async clipboard backend trait + no-op default.
//!
//! Backends implement [`ClipboardBackend`]; the [`crate::Clipboard`]
//! capability newtype wraps an `Rc<dyn ClipboardBackend>` for registry
//! storage. The trait is object-safe — methods return boxed `Future`s so a
//! `dyn ClipboardBackend` can drive async operations.

use std::future::Future;
use std::pin::Pin;

/// Async clipboard backend. Methods return boxed `Future`s because the
/// trait is held as `dyn ClipboardBackend` (object-safe). Backends decide
/// whether the operation is actually async — sync backends (e.g. a test
/// stub) return `std::future::ready(...)`.
///
/// Backends are registered as the [`crate::Clipboard`] capability via
/// `tur_engine::TurEngineBuilder::capability(Clipboard::new(backend))`.
/// Bridge fns look up the cap at JS call time and call these methods.
///
/// On wasm, `navigator.clipboard.readText/writeText` are inherently async
/// (return JS Promises); on native/tests, this can resolve eagerly.
pub trait ClipboardBackend: 'static {
    /// Read text from the clipboard. Resolves with the text (empty string
    /// if denied/unavailable).
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>>;

    /// Write text to the clipboard. Resolves when the write has been
    /// acknowledged by the platform.
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>>;
}

/// No-op `ClipboardBackend` default. Reads return empty string; writes drop
/// the text. Used when no platform clipboard is injected (e.g. minimal
/// tests).
#[derive(Default)]
pub struct NoopClipboard;
impl ClipboardBackend for NoopClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        Box::pin(std::future::ready(String::new()))
    }
    fn write_text(&self, _text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(std::future::ready(()))
    }
}
