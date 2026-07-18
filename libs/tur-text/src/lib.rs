//! Text feature library for tur.
//!
//! Provides text rendering and editing elements (`TextElement`,
//! `EditableTextElement`, `ParagraphElement`), their controllers
//! (`TextEditingController`, `UndoController`), the post-event
//! `EnsureCaretVisibleHandler`, and the `extract_layout_data` bridge helper.
//!
//! Unlike `tur-animation`, this crate is **not** a plugin. It is installed
//! into `builtin:tur/std` by `TurStdPlugin` via [`install_text_feature`],
//! which registers the boa classes + post-handler and returns the JS factory
//! fns to be merged into `std_fns`. From JS's perspective Text/Input ship as
//! part of `builtin:tur/std`.
//!
//! The engine retains only the paint/layout contract types —
//! `tur_engine::core::text::TextLayoutData` and
//! `tur_engine::core::fonts::FontManager` — which `Canvas::fill_text_layout`
//! consumes to do the actual drawing. tur-text produces these structs from
//! JS-side props via `extract_layout_data`.

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
/// - Registers tur-text's post-handlers ([`handlers::EnsureCaretVisibleHandler`])
///   so the caret stays visible after keyboard / IME / clipboard-paste events.
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

    // Runs after engine's KeyboardAppHandler / ImeAppHandler /
    // ClipboardPasteAppHandler (registration order is preserved by the
    // engine's handler vec). Keeps the caret visible after any text-moving
    // event; no-op when the focused element isn't an EditableText.
    ctx.register_handler(Box::new(handlers::EnsureCaretVisibleHandler));

    let mut fns = Vec::new();
    fns.extend(elements::paragraph::bridge::fns());
    fns.extend(elements::editable_text::bridge::fns());
    Ok(fns)
}
