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
//! - Clipboard event payloads ([`events::ClipboardPlatformPasteEvent`],
//!   [`events::ClipboardPasteEvent`], [`events::ClipboardWriteEvent`]) that
//!   travel inside the engine's `PlatformEvent::Custom` / `AppEvent::Custom`
//!   variants, plus queue helpers ([`events::platform_paste`],
//!   [`events::push_paste`], [`events::push_write`]).
//! - The engine-internal subsystems ([`ClipboardPlatformSubsystem`] for
//!   embedder → engine paste forwarding, [`ClipboardWriteSubsystem`] for the
//!   Cmd+C/Cmd+X write path), both registered by [`TurClipboardPlugin`] so
//!   embedders no longer need to wire clipboard handling through
//!   `TurStdPlugin` separately.
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
pub mod events;
pub mod handlers;
pub mod platform;

use tur_engine::core::capability::Capability;
use tur_engine::core::capability::CapabilityDecls;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::bridge::helpers::ConstEntry;
use tur_engine::error::TurError;

pub use platform::{ClipboardBackend, NoopClipboard};
pub use handlers::{ClipboardPlatformSubsystem, ClipboardWriteSubsystem};
pub use events::{
    ClipboardPasteEvent, ClipboardPlatformPasteEvent, ClipboardWriteEvent,
    platform_paste, push_paste, push_write,
};

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
/// engine-internal [`ClipboardPlatformSubsystem`] (forwards embedder paste
/// into the engine-internal event bus) and [`ClipboardWriteSubsystem`] (the
/// Cmd+C/Cmd+X event path).
///
/// Both subsystems route through the engine's `Custom` event variants:
/// embedders wrap their paste as a [`ClipboardPlatformPasteEvent`] via
/// [`platform_paste`]; tur-text consumes the forwarded
/// [`ClipboardPasteEvent`] and produces [`ClipboardWriteEvent`] on
/// copy/cut via [`push_write`].
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
        // Engine-internal subsystems.
        //
        // `ClipboardPlatformSubsystem` must run BEFORE tur-text's
        // `ClipboardPasteSubsystem` in the AppEvent drain pass: it produces
        // the `ClipboardPasteEvent` (App) that tur-text consumes. Because
        // AppEvents drain on a later fixed-point iteration than the
        // originating PlatformEvent (queues are snapshotted at the start of
        // `flush_app_events`), the subsystem registration order within a
        // single iteration doesn't actually gate correctness here — but
        // registering it first matches the data-flow direction.
        //
        // `ClipboardWriteSubsystem` looks up the Clipboard capability at
        // dispatch time via `cx.capabilities.of::<Clipboard>()` — so if the
        // cap is missing (which the `requires` declaration above should
        // have caught at build()), writes silently drop with a warning.
        ctx.register_subsystem(Box::new(ClipboardPlatformSubsystem));
        ctx.register_subsystem(Box::new(ClipboardWriteSubsystem));

        // Build the `clipboard` object (with `readText`/`writeText` methods)
        // and register it as the module's only export.
        let ctx_value = ctx.js_ctx_value.clone();
        let clipboard_obj = bridge::build_clipboard_object(ctx.boa_mut(), ctx_value);
        let consts: Vec<ConstEntry> = vec![("clipboard", clipboard_obj)];

        ctx.register_module("builtin:tur/clipboard", bridge::fns(), vec![], consts);

        Ok(())
    }
}
