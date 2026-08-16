//! `ClipboardBackend` impl backed by the `arboard` crate, with each OS call
//! hopped onto the engine's main thread via an [`HostExecutor`].
//!
//! `arboard` is synchronous and, on macOS, talks to AppKit's `NSPasteboard`
//! — which requires main-thread access. Since `flush()` (and thus the bridge
//! that calls this backend) runs on the worker thread after the
//! worker-owns-paint refactor, each `read_text` / `write_text` posts its
//! `arboard` call to the main thread via `cx.run_on_host(...)` and awaits the
//! result on the worker. On platforms without a main-thread requirement the
//! hop is a harmless extra channel round-trip.
//!
//! The backend receives its [`HostExecutor`] at construction, via the
//! closure form of `TurRuntimeBuilder::capability` (the engine creates the
//! channel internally in `build()` and passes the context to the closure).
//!
//! [`HostExecutor`]: tur_engine::HostExecutor

use std::future::Future;
use std::pin::Pin;

use arboard::Clipboard as ArboardClipboard;
use tur_engine::error::TurError;
use tur_engine::{ClipboardBackend, HostExecutor};

/// Native clipboard backend. Wraps an [`HostExecutor`] used to hop each
/// read/write onto the main thread (`arboard` is synchronous; macOS
/// `NSPasteboard` requires main-thread access).
///
/// Construction can fail on systems without a clipboard (e.g. headless CI).
/// Embedders should fall back to the test stub (or skip registering the
/// capability) when [`NativeClipboard::new`] returns `Err`.
///
/// Register via the closure form of `TurRuntimeBuilder::capability`:
///
/// ```no_run
/// # use tur_clipboard_native::{Clipboard, NativeClipboard};
/// # use tur_engine::TurRuntimeBuilder;
/// TurRuntimeBuilder::new()
///     .capability(|cx| Ok(Clipboard::new(NativeClipboard::new(cx)?)))
///     // .plugin(TurClipboardPlugin) ...
/// # ;
/// ```
pub struct NativeClipboard {
    cx: HostExecutor,
}

impl NativeClipboard {
    /// Open the platform clipboard and capture the engine's async context
    /// (cloned internally).
    ///
    /// The engine calls this in `build()` (which runs on the main thread), so
    /// the `arboard` constructor (which touches AppKit on macOS) runs on main.
    /// Construction fails on systems without a clipboard (e.g. headless CI).
    pub fn new(cx: &HostExecutor) -> Result<Self, TurError> {
        // Validate that a clipboard is available (fails on headless CI). The
        // handle is dropped — each method opens a fresh one below because
        // `arboard::Clipboard::get_text`/`set_text` take `&mut self` while
        // the trait methods are `&self`.
        ArboardClipboard::new()
            .map(|_| Self { cx: cx.clone() })
            .map_err(|e| TurError::Other(format!("arboard clipboard unavailable: {e}")))
    }
}

impl ClipboardBackend for NativeClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        let cx = self.cx.clone();
        Box::pin(async move {
            // Hop the `arboard` call onto the main thread and await the
            // result on the worker. `Err(Dropped)` ⇒ the drain was dropped
            // (engine shutting down) — resolve empty like the error path.
            match cx
                .run_on_host(|| ArboardClipboard::new().and_then(|mut cb| cb.get_text()))
                .await
            {
                Ok(Ok(text)) => text,
                Ok(Err(e)) => {
                    tracing::warn!("clipboard read failed: {e}");
                    String::new()
                }
                Err(_) => {
                    tracing::warn!("clipboard read: main-thread drain gone");
                    String::new()
                }
            }
        })
    }

    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        let cx = self.cx.clone();
        Box::pin(async move {
            match cx
                .run_on_host(move || ArboardClipboard::new().and_then(|mut cb| cb.set_text(&text)))
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("clipboard write failed: {e}"),
                Err(_) => tracing::warn!("clipboard write: main-thread drain gone"),
            }
        })
    }
}
