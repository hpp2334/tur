//! Clipboard plugin — backend trait + capability newtype + event payloads +
//! `tur:clipboard` JS bridge + engine-internal subsystems.
//!
//! Inlined from the former `tur-clipboard-capability` crate. Exposes a
//! minimal public API surface; the rest is internal to `builtin_plugins`.
//!
//! ## Public API (re-exported at the `tur_engine` crate root)
//!
//! - [`Clipboard`] — capability newtype; embedders register via
//!   `Clipboard::new(backend)` on `TurRuntimeBuilder::capability`.
//! - [`ClipboardBackend`] — trait external backends implement
//!   (`tur-clipboard-wasm::WasmClipboard`, `tur-clipboard-native::NativeClipboard`).
//! - [`TurClipboardPlugin`] — plugin struct embedders register via
//!   `TurRuntimeBuilder::plugin`.
//! - [`platform_paste`] — embedder helper wrapping a paste text as a
//!   `PlatformEvent::Custom`.
//!
//! ## Internal to `builtin_plugins`
//!
//! - Event payload types + `push_paste` / `push_write` helpers (used by this
//!   plugin and by `builtin_plugins/text`).
//! - [`ClipboardPlatformSubsystem`] / [`ClipboardWriteSubsystem`] engine
//!   event-bus handlers.
//! - The JS bridge fns (ctx-bound `Ptr`s) — registered as the
//!   `clipboard.readText` / `clipboard.writeText` consts of
//!   `tur:clipboard`.
//!
//! [`ClipboardPlatformSubsystem`]: handlers::ClipboardPlatformSubsystem
//! [`ClipboardWriteSubsystem`]: handlers::ClipboardWriteSubsystem

pub(in crate::builtin_plugins) mod bridge;
pub mod capability;
pub(in crate::builtin_plugins) mod event;
pub(in crate::builtin_plugins) mod handlers;

pub use capability::{Clipboard, ClipboardBackend};
pub use event::platform_paste;
pub(in crate::builtin_plugins) use event::{ClipboardPasteEvent, push_write};

use crate::core::capability::CapabilityDecls;
use crate::core::js_runtime::helpers::ConstEntry;
use crate::core::plugin::{Plugin, PluginRegisterContext};
use crate::error::TurError;

/// tur-clipboard plugin: registers `tur:clipboard` (exporting a
/// `clipboard` object with `readText` / `writeText` methods) plus the
/// engine-internal [`ClipboardPlatformSubsystem`](handlers::ClipboardPlatformSubsystem)
/// (forwards embedder paste into the engine-internal event bus) and
/// [`ClipboardWriteSubsystem`](handlers::ClipboardWriteSubsystem) (the
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

    fn register(&self, ctx: &mut PluginRegisterContext<'_>) -> Result<(), TurError> {
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
        ctx.register_subsystem(Box::new(handlers::ClipboardPlatformSubsystem));
        ctx.register_subsystem(Box::new(handlers::ClipboardWriteSubsystem));

        // Build the `clipboard` object (with `readText`/`writeText` methods)
        // and register it as the module's only export.
        let ctx_value = ctx.js_ctx_value.clone();
        let clipboard_obj = bridge::build_clipboard_object(ctx.boa_mut(), ctx_value);
        let consts: Vec<ConstEntry> = vec![("clipboard", clipboard_obj)];

        ctx.register_module("tur:clipboard", bridge::fns(), consts);

        Ok(())
    }
}
