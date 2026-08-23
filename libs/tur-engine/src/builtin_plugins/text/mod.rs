//! Text plugin — text rendering and editing.
//!
//! Provides text rendering and editing elements (`TextElement`,
//! `EditableTextElement`, `ParagraphElement`), their controllers
//! (`TextEditingController`, `UndoController`), the paste + caret-visible
//! subsystems (`ClipboardPasteSubsystem`, `CaretVisibilitySubsystem`), and
//! the `extract_layout_data` bridge helper.
//!
//! Installed into `tur:std` by `TurStdPlugin` via [`install_text`],
//! which registers the boa classes + subsystems and returns the JS factory
//! fns to be merged into `std_fns`. From JS's perspective Text/Input ship
//! as part of `tur:std`.
//!
//! The engine retains only the paint/layout contract types —
//! `crate::core::text::TextLayoutData` and `crate::core::fonts::FontManager`
//! — which `Canvas::fill_text_layout` consumes to do the actual drawing.
//! This plugin produces these structs from JS-side props via
//! `extract_layout_data`. Paste flows through the engine-internal bus:
//! tur-clipboard's `ClipboardPlatformSubsystem` (registered by
//! `TurClipboardPlugin`) forwards the embedder's
//! `ClipboardPlatformPasteEvent` (PlatformEvent::Custom) as a
//! `ClipboardPasteEvent` (AppEvent::Custom), which
//! [`handlers::ClipboardPasteSubsystem`] consumes here.

pub mod controller;
pub mod elements;
pub mod handlers;
pub mod text_layout;

pub use controller::{TextEditingController, UndoController};
pub use elements::{EditableTextElement, EditableTextView, InputView, TextElement, TextView};

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

/// Wire text plugin into `tur:std`. Called by `TurStdPlugin`'s
/// `register` impl.
///
/// Side effects:
/// - Registers the boa classes [`TextEditingController`] and
///   [`UndoController`] on `globalThis`.
/// - Registers this plugin's [`handlers::ClipboardPasteSubsystem`] (consumes
///   a `ClipboardPasteEvent` — AppEvent::Custom — forwarded by
///   tur-clipboard's `ClipboardPlatformSubsystem`) and
///   [`handlers::CaretVisibilitySubsystem`] (post-subsystem that keeps the
///   caret visible after keyboard / IME / paste events). Registration order
///   matters: paste subsystem before caret-visible subsystem, so the latter
///   observes the post-paste caret.
///
/// Returns: the `Text` / `Input` / `createTextEditingController` /
/// `createUndoController` factory fns, which the caller merges into
/// `std_fns` before `register_module("tur:std", ...)`.
pub fn install_text(ctx: &mut PluginRegisterContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_class::<TextEditingController>()
        .map_err(|e| TurError::Other(format!("failed to register TextEditingController: {e}")))?;
    ctx.register_class::<UndoController>()
        .map_err(|e| TurError::Other(format!("failed to register UndoController: {e}")))?;

    // Subsystems run in registration order, so register the paste subsystem
    // BEFORE `CaretVisibilitySubsystem`. Both consume a
    // `ClipboardPasteEvent` (AppEvent::Custom): paste mutates the focused
    // editable's text + caret, then the caret-visible subsystem observes the
    // post-paste caret and scrolls if needed. (Engine's `KeyboardSubsystem`
    // / `ImeSubsystem` are registered even earlier by `TurStdPlugin`, so
    // keyboard / IME caret moves also land before `CaretVisibilitySubsystem`.)
    ctx.register_subsystem(Box::new(handlers::ClipboardPasteSubsystem));
    ctx.register_subsystem(Box::new(handlers::CaretVisibilitySubsystem));

    let mut fns = Vec::new();
    fns.extend(elements::paragraph::bridge::fns());
    fns.extend(elements::editable_text::bridge::fns());
    Ok(fns)
}
