//! `ClipboardBackend` impl backed by the `arboard` crate. `arboard` is
//! synchronous (it talks to the OS clipboard API directly), so both methods
//! resolve eagerly via `std::future::ready`. Errors are logged and the
//! methods return empty/`()` to match the trait's "best-effort" contract.

use std::future::Future;
use std::pin::Pin;

use arboard::Clipboard as ArboardClipboard;
use tur_engine::ClipboardBackend;

/// Native clipboard backend. Wraps an [`arboard::Clipboard`] handle.
///
/// Construction can fail on systems without a clipboard (e.g. headless CI).
/// Embedders should fall back to the test stub (or skip registering the
/// capability) when [`NativeClipboard::new`] returns `Err`.
///
/// Note: the `arboard::Clipboard` field isn't read directly because the
/// trait methods are `&self` while `arboard::Clipboard::get_text` /
/// `set_text` take `&mut`. Each method opens a fresh handle internally —
/// the field is kept only as a marker that construction succeeded.
pub struct NativeClipboard(());

impl NativeClipboard {
    /// Open the platform clipboard. Fails on systems without one.
    pub fn new() -> Result<Self, arboard::Error> {
        ArboardClipboard::new().map(|_| Self(()))
    }
}

impl ClipboardBackend for NativeClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        // `arboard::Clipboard::get_text` borrows `&self` mutably (some
        // platforms open/close the clipboard around each call). We can't
        // take `&mut` here (the trait method is `&self`), so we open a
        // fresh handle for each read — cheap, and matches how the Wasm
        // backend treats each call as independent.
        let result = ArboardClipboard::new()
            .and_then(|mut cb| cb.get_text())
            .unwrap_or_else(|e| {
                tracing::warn!("clipboard read failed: {e}");
                String::new()
            });
        Box::pin(std::future::ready(result))
    }

    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        if let Err(e) = ArboardClipboard::new().and_then(|mut cb| cb.set_text(&text)) {
            tracing::warn!("clipboard write failed: {e}");
        }
        Box::pin(std::future::ready(()))
    }
}
