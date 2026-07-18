//! Text feature library for tur.
//!
//! Provides text rendering and editing elements (`TextElement`,
//! `EditableTextElement`, `ParagraphElement`), their controllers
//! (`TextEditingController`, `UndoController`), the paste + caret-visible
//! handlers (`ClipboardPasteHandler`, `EnsureCaretVisibleHandler`), and the
//! `extract_layout_data` bridge helper.
//!
//! Unlike `tur-animation`, this crate is **not** a plugin. It is installed
//! into `builtin:tur/std` by `TurStdPlugin` via [`install_text_feature`],
//! which registers the boa classes + handlers and returns the JS factory
//! fns to be merged into `std_fns`. From JS's perspective Text/Input ship as
//! part of `builtin:tur/std`.
//!
//! The engine retains only the paint/layout contract types —
//! `tur_engine::core::text::TextLayoutData` and
//! `tur_engine::core::fonts::FontManager` — which `Canvas::fill_text_layout`
//! consumes to do the actual drawing. tur-text produces these structs from
//! JS-side props via `extract_layout_data`. Paste flows through the
//! engine-internal bus: the engine's `ClipboardPasteAppHandler` (in
//! `TurStdPlugin`) forwards `PlatformEvent::ClipboardPaste` as
//! `AppEvent::ClipboardPaste`, which [`handlers::ClipboardPasteHandler`]
//! consumes here.

pub mod controller;
pub mod elements;
pub mod handlers;
pub mod text_layout;

pub use controller::{TextEditingController, UndoController};
pub use elements::{EditableTextElement, EditableTextView, InputView, TextElement, TextView};

use tur_engine::core::bridge::helpers::FnEntry;
use tur_engine::core::plugin::PluginContext;
use tur_engine::error::TurError;

/// Wire text feature into `builtin:tur/std`. Called by `TurStdPlugin`'s
/// `register` impl — text is not a separate plugin, it's a feature installed
/// into the std module.
///
/// Side effects:
/// - Registers the boa classes [`TextEditingController`] and
///   [`UndoController`] on `globalThis`.
/// - Registers tur-text's [`handlers::ClipboardPasteHandler`] (consumes
///   `AppEvent::ClipboardPaste` forwarded by the engine's
///   `ClipboardPasteAppHandler`) and [`handlers::EnsureCaretVisibleHandler`]
///   (post-handler that keeps the caret visible after keyboard / IME / paste
///   events). Registration order matters: paste handler before caret-visible
///   handler, so the latter observes the post-paste caret.
///
/// Returns: the `Text` / `Input` / `createTextEditingController` /
/// `createUndoController` factory fns, which the caller merges into
/// `std_fns` before `register_module("builtin:tur/std", ...)`.
pub fn install_text_feature(
    ctx: &mut PluginContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_class::<TextEditingController>()
        .map_err(|e| TurError::Other(format!("failed to register TextEditingController: {e}")))?;
    ctx.register_class::<UndoController>()
        .map_err(|e| TurError::Other(format!("failed to register UndoController: {e}")))?;

    // Handlers run in registration order, so register the paste handler
    // BEFORE `EnsureCaretVisibleHandler`. Both consume
    // `AppEvent::ClipboardPaste`: paste mutates the focused editable's text +
    // caret, then the caret-visible handler observes the post-paste caret
    // and scrolls if needed. (Engine's `KeyboardAppHandler` /
    // `ImeAppHandler` are registered even earlier by `TurStdPlugin`, so
    // keyboard / IME caret moves also land before `EnsureCaretVisibleHandler`.)
    ctx.register_handler(Box::new(handlers::ClipboardPasteHandler));
    ctx.register_handler(Box::new(handlers::EnsureCaretVisibleHandler));

    let mut fns = Vec::new();
    fns.extend(elements::paragraph::bridge::fns());
    fns.extend(elements::editable_text::bridge::fns());
    Ok(fns)
}
