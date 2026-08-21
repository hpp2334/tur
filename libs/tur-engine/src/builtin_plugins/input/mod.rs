//! Input plugin — keyboard + IME subsystems that route events to the
//! focused element.
//!
//! - [`KeyboardSubsystem`] — synchronously dispatches key events to the
//!   focused element (`ElementOnKeyboard::on_keyboard_event`), then walks the
//!   focus chain bubbling `onKeyDown$` mutations on `Focusable` ancestors.
//! - [`ImeSubsystem`] — routes `ShellEvent::Ime` to the focused element
//!   (typically a tur-text `EditableTextElement`). The element owns the text
//!   editing logic; this subsystem just forwards the event.
//!
//! The cross-plugin dependency on `FocusableElement` (for the keyboard bubble
//! phase) is one-way: input reads focus state + casts to Focusable, but focus
//! knows nothing about input.
//!
//! The event payload types themselves (`KeyEvent`, `KeyEventType`,
//! `Modifiers`, `KeydownEvent`, `KeyupEvent`) live in
//! `crate::core::platform::key_event` (engine contract types) since
//! `ShellEvent::Key` wraps them.

pub(in crate::builtin_plugins) mod ime;
pub(in crate::builtin_plugins) mod subsystem;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

pub(in crate::builtin_plugins) use ime::ImeSubsystem;
pub(in crate::builtin_plugins) use subsystem::KeyboardSubsystem;

/// Install the input plugin (`KeyboardSubsystem`, `ImeSubsystem`) — no JS
/// factory fns are returned (this plugin contributes subsystems only). The
/// orchestrator merges the empty fn list into `tur:std` harmlessly.
pub fn install_input(ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_subsystem(Box::new(KeyboardSubsystem));
    ctx.register_subsystem(Box::new(ImeSubsystem));
    Ok(Vec::new())
}
