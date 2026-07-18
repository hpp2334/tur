//! Async clipboard capability for tur.
//!
//! Provides:
//!
//! - The [`ClipboardBackend`] trait + [`NoopClipboard`] default (backends
//!   implement `ClipboardBackend`).
//! - The [`Clipboard`] capability newtype that wraps `Rc<dyn ClipboardBackend>`
//!   and is registered via
//!   [`tur_engine::TurEngineBuilder::capability`](`Clipboard::new(backend)`).
//! - The `builtin:tur/clipboard` bridge module (exporting a `clipboard`
//!   object with `readText` / `writeText` methods, both Promise-returning).
//! - The engine-internal [`ClipboardWriteHandler`] (Cmd+C/Cmd+X event path),
//!   registered by [`TurClipboardPlugin`] so embedders no longer need to wire
//!   the clipboard backend through `TurStdPlugin` separately.
//!
//! Paste (Cmd+V) is not handled here — it flows through the engine's standard
//! element-event pipeline as `PlatformEvent::ClipboardPaste`, dispatched to
//! the focused element's `ElementOnClipboard` impl by the engine's
//! `ClipboardPasteAppHandler` (registered by `TurStdPlugin`).
//!
//! ## Architecture
//!
//! - Backends (`WasmClipboard` in `tur-clipboard-wasm`, `NativeClipboard` in
//!   `tur-clipboard-native`, `RecordingClipboard` in `tur-integration-tests`)
//!   are registered via `.capability(Clipboard::new(backend))` on the engine
//!   builder.
//! - Bridge fns (in [`bridge`]) are ctx-bound `Ptr`s that look up the
//!   [`Clipboard`] capability via `js_ctx.capability().of::<Clipboard>()` at
//!   JS call time. No `unsafe` closures.
//! - [`TurClipboardPlugin`] is a unit struct — `requires` declares
//!   `Clipboard`, so the engine builder fails fast if the embedder forgot
//!   to register a backend.

pub mod bridge;
pub mod handlers;
pub mod platform;

use tur_engine::core::capability::Capability;
use tur_engine::core::capability::CapabilityDecls;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::bridge::helpers::ConstEntry;
use tur_engine::error::TurError;

pub use platform::{ClipboardBackend, NoopClipboard};
pub use handlers::ClipboardWriteHandler;

/// Capability newtype wrapping an `Rc<dyn ClipboardBackend>`. Registered via
/// [`tur_engine::TurEngineBuilder::capability`] with
/// `Clipboard::new(backend)`; bridge fns look it up at JS call time via
/// `js_ctx.capability().of::<Clipboard>()`.
#[derive(Clone)]
pub struct Clipboard(std::rc::Rc<dyn ClipboardBackend>);

impl Clipboard {
    /// Wrap a backend in the capability newtype.
    pub fn new(backend: impl ClipboardBackend + 'static) -> Self {
        Self(std::rc::Rc::new(backend))
    }

    /// Borrow the underlying backend handle.
    pub fn backend(&self) -> &std::rc::Rc<dyn ClipboardBackend> {
        &self.0
    }
}

impl Capability for Clipboard {}

/// tur-clipboard plugin: registers `builtin:tur/clipboard` (exporting a
/// `clipboard` object with `readText` / `writeText` methods) plus the
/// engine-internal `ClipboardWriteHandler` (for the Cmd+C/Cmd+X event path).
///
/// Paste (Cmd+V) is handled separately by the engine's
/// `ClipboardPasteAppHandler`, dispatched to the focused element's
/// `ElementOnClipboard` impl.
///
/// The plugin declares a hard dependency on the [`Clipboard`] capability
/// via `requires`; the engine builder fails fast at `build()` if the
/// embedder forgot to register a backend via
/// `.capability(Clipboard::new(...))`.
///
/// The plugin itself is a unit struct — no builder.
pub struct TurClipboardPlugin;

impl Default for TurClipboardPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurClipboardPlugin {
    fn requires(&self, decls: &mut CapabilityDecls) {
        decls.need::<Clipboard>();
    }

    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // Engine-internal event handler for Cmd+C / Cmd+X. Looks up the
        // Clipboard capability at dispatch time via
        // `cx.capabilities.of::<Clipboard>()` — so if the cap is missing
        // (which the `requires` declaration above should have caught at
        // build()), writes silently drop with a warning.
        ctx.register_handler(Box::new(ClipboardWriteHandler));

        // Build the `clipboard` object (with `readText`/`writeText` methods)
        // and register it as the module's only export.
        let ctx_value = ctx.js_ctx_value.clone();
        let clipboard_obj = bridge::build_clipboard_object(ctx.boa_mut(), ctx_value);
        let consts: Vec<ConstEntry> = vec![("clipboard", clipboard_obj)];

        ctx.register_module("builtin:tur/clipboard", bridge::fns(), vec![], consts);

        Ok(())
    }
}
