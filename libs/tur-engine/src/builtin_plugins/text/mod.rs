//! Text plugin — text rendering and editing.
//!
//! Provides text rendering and editing elements (`TextElement`,
//! `EditableTextElement`, `ParagraphElement`), their controllers
//! (`TextEditingController`, `UndoController` — plain Rust types), the
//! paste + caret-visible subsystems (`ClipboardPasteSubsystem`,
//! `CaretVisibilitySubsystem`), and the `extract_layout_data` bridge
//! helper.
//!
//! Installed by `TurStdPlugin` via [`install_text`], which registers the
//! subsystems. Elements materialize pure-Rust views (authored through the
//! `core::rut_runtime` rows); controllers are shared `Rc<RefCell<...>>`.
//!
//! The engine retains only the paint/layout contract types —
//! `crate::core::text::TextLayoutData` and `crate::core::fonts::FontManager`
//! — which `Canvas::fill_text_layout` consumes to do the actual drawing.
//! This plugin produces these structs from controller/view state via
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


use crate::error::TurError;

/// Wire the text plugin's subsystems in. Called by `TurStdPlugin`'s
/// `register` impl.
///
/// Side effects — subsystem registration only (order matters):
/// - [`handlers::ClipboardPasteSubsystem`] BEFORE
///   [`handlers::CaretVisibilitySubsystem`]. Both consume a
///   `ClipboardPasteEvent` (AppEvent::Custom): paste mutates the focused
///   editable's text + caret, then the caret-visible subsystem observes the
///   post-paste caret and scrolls if needed. (Engine's `KeyboardSubsystem`
///   / `ImeSubsystem` are registered even earlier by `TurStdPlugin`, so
///   keyboard / IME caret moves also land before `CaretVisibilitySubsystem`.)
use crate::core::plugin::PluginRegisterContext;
pub fn install_text(ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
    ctx.register_subsystem(Box::new(handlers::ClipboardPasteSubsystem));
    ctx.register_subsystem(Box::new(handlers::CaretVisibilitySubsystem));
    Ok(())
}
