//! Platform capability traits for tur-std plugins.
//!
//! These describe host services the engine can't provide itself (cursor
//! output, async clipboard, async HTTP). Each is injected through
//! [`crate::TurStdPluginBuilder`] and stored on the plugin.

use std::future::Future;
use std::pin::Pin;

use tur_shared::Cursor;

/// Cursor output: the engine pushes the resolved cursor during the frame
/// loop, and the platform applies it (e.g. set the host canvas CSS cursor
/// in tur-wasm).
pub trait CursorPlatform {
    fn set_cursor(&mut self, cursor: Cursor);
}

/// Async clipboard capability. Methods return boxed `Future`s because the
/// trait is held as `dyn Clipboard` (object-safe). Backends decide whether
/// the operation is actually async — sync backends (e.g. a test stub)
/// return `std::future::ready(...)`.
///
/// Injected via [`crate::TurStdPluginBuilder::clipboard`]; the bridge fns
/// `clipboardReadText` / `clipboardWriteText` in `builtin:tur/std` consume
/// it. On wasm, `navigator.clipboard.readText/writeText` are inherently
/// async (return JS Promises); on native/tests, this can resolve eagerly.
pub trait Clipboard: 'static {
    /// Read text from the clipboard. Resolves with the text (empty string
    /// if denied/unavailable).
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>>;

    /// Write text to the clipboard. Resolves when the write has been
    /// acknowledged by the platform.
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>>;
}

/// No-op `CursorPlatform` default.
pub struct NoopCursorPlatform;
impl CursorPlatform for NoopCursorPlatform {
    fn set_cursor(&mut self, _cursor: Cursor) {}
}

/// No-op `Clipboard` default. Reads return empty string; writes drop the
/// text. Used when no platform clipboard is injected (e.g. minimal tests).
#[derive(Default)]
pub struct NoopClipboard;
impl Clipboard for NoopClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        Box::pin(std::future::ready(String::new()))
    }
    fn write_text(&self, _text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(std::future::ready(()))
    }
}
