//! `ClipboardBackend` impl backed by `navigator.clipboard`. Reads await a
//! browser Promise (`navigator.clipboard.readText()`); writes are
//! fire-and-forget `writeText` calls.
//!
//! The calls NEVER run here directly in production: the engine's pump (which
//! drives this backend's capability futures) lives on a Web Worker, and
//! `navigator.clipboard` is `Window`-only. Both ops hop to the main thread
//! over the tagged-message bridge (`bridge` — the boa era's host-bridge /
//! frame-loop-drain shape). On a main-thread embedding (no worker) the
//! direct `Window` call is used.

use std::future::Future;
use std::pin::Pin;

use tur_engine::ClipboardBackend;

use super::bridge;

/// Browser clipboard backend. Wraps `navigator.clipboard.readText` /
/// `writeText` — both promise-based JS calls (surfaced to tur JS as the
/// `Task`-returning `clipboard` object).
#[derive(Default)]
pub struct WasmClipboard;

impl ClipboardBackend for WasmClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        Box::pin(async move {
            // Worker (production): round-trip to the main thread.
            if let Some(pending) = bridge::worker_begin_read() {
                return pending.await;
            }
            // Main-thread embedding: direct call.
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
            // Worker (production): hop to the main thread — fire-and-forget
            // matches the boa-era host bridge.
            if bridge::worker_post_write(&text) {
                return;
            }
            if let Some(window) = web_sys::window() {
                let _ = window.navigator().clipboard().write_text(&text);
            }
        })
    }
}
