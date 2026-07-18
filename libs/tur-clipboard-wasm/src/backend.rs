//! `ClipboardBackend` impl backed by `navigator.clipboard`. Reads await a
//! browser Promise (`navigator.clipboard.readText()`); writes are
//! fire-and-forget `writeText` calls but still go through the executor so
//! the engine sees consistent spawn → poll → complete flow.

use std::future::Future;
use std::pin::Pin;

use tur_clipboard_capability::ClipboardBackend;

/// Browser clipboard backend. Wraps `navigator.clipboard.readText` /
/// `writeText` — both Promise-returning JS calls.
#[derive(Default)]
pub struct WasmClipboard;

impl ClipboardBackend for WasmClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        Box::pin(async {
            let Some(window) = web_sys::window() else {
                return String::new();
            };
            let promise = window.navigator().clipboard().read_text();
            match wasm_bindgen_futures::JsFuture::from(promise).await {
                Ok(v) => v.as_string().unwrap_or_default(),
                Err(_) => String::new(),
            }
        })
    }
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            if let Some(window) = web_sys::window() {
                let _ = window.navigator().clipboard().write_text(&text);
            }
        })
    }
}
