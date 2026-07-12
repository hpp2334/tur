//! Async clipboard plugin for tur.
//!
//! Provides the `builtin:tur/clipboard` bridge module exporting a `clipboard`
//! object with `readText()` / `writeText()` methods (both Promise-returning).
//!
//! ## Architecture
//!
//! - The [`Clipboard`] trait + `NoopClipboard` live in `tur-std` (alongside
//!   the engine's paste/write event handlers). tur-clipboard depends on
//!   tur-std for the trait.
//! - The bridge fns (in [`bridge`]) are ctx-bound `Ptr`s that look up their
//!   `Rc<dyn Clipboard>` and `Rc<AsyncExecutor>` from `TurJsContext`'s
//!   capability registry — populated by this plugin during `register` (and
//!   by the engine for the executor). No `unsafe` closures.
//! - Backends (`WasmClipboard` in tur-wasm, `RecordingClipboard` in tests)
//!   inject their impl via [`TurClipboardPlugin::builder`].
//!
//! Note: the same `Rc<dyn Clipboard>` should also be passed to
//! [`tur_std::TurStdPlugin`] — tur-std's `ClipboardWriteHandler` and
//! `ClipboardPasteHandler` use it for the Cmd+C/Cmd+V/Cmd+X event path
//! (engine-internal), while this plugin exposes the JS-callable bridge.

pub mod bridge;

use std::rc::Rc;

use tur_engine::core::bridge::helpers::ConstEntry;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

use tur_std::{Clipboard, NoopClipboard};

pub use tur_std::{Clipboard as ClipboardTrait, NoopClipboard as NoopClipboardBackend};

/// tur-clipboard plugin: registers `builtin:tur/clipboard` (exporting a
/// `clipboard` object with `readText` / `writeText` methods) and exposes the
/// injected [`Clipboard`] backend to ctx-bound bridge fns via `TurJsContext`'s
/// capability registry.
pub struct TurClipboardPlugin {
    clipboard: Rc<dyn Clipboard>,
}

impl TurClipboardPlugin {
    pub fn builder() -> TurClipboardPluginBuilder {
        TurClipboardPluginBuilder::new()
    }
}

impl Default for TurClipboardPlugin {
    fn default() -> Self {
        Self {
            clipboard: Rc::new(NoopClipboard),
        }
    }
}

impl Plugin for TurClipboardPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // Expose the clipboard backend to ctx-bound bridge fns. The executor
        // capability is already inserted by the engine.
        ctx.js_ctx()
            .insert_capability::<Rc<dyn Clipboard>>(self.clipboard.clone());

        // Build the `clipboard` object (with `readText`/`writeText` methods)
        // and register it as the module's only export.
        let ctx_value = ctx.js_ctx_value.clone();
        let clipboard_obj = bridge::build_clipboard_object(ctx.boa_mut(), ctx_value);
        let consts: Vec<ConstEntry> = vec![("clipboard", clipboard_obj)];

        ctx.register_module("builtin:tur/clipboard", bridge::fns(), vec![], consts);

        Ok(())
    }
}

pub struct TurClipboardPluginBuilder {
    clipboard: Option<Rc<dyn Clipboard>>,
}

impl Default for TurClipboardPluginBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TurClipboardPluginBuilder {
    pub fn new() -> Self {
        Self { clipboard: None }
    }

    /// Inject a clipboard backend. When set, the plugin registers
    /// `builtin:tur/clipboard` (with the `clipboard` object) during `register`.
    pub fn clipboard<P: Clipboard + 'static>(mut self, platform: P) -> Self {
        self.clipboard = Some(Rc::new(platform));
        self
    }

    pub fn build(self) -> TurClipboardPlugin {
        TurClipboardPlugin {
            clipboard: self.clipboard.unwrap_or_else(|| Rc::new(NoopClipboard)),
        }
    }
}
