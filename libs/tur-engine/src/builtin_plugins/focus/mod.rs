//! Focus widget plugin:
//! - `Focusable` element + `requestFocus` bridge fn.
//!
//! Key-event dispatch and bubble-up live in the sibling `input` plugin
//! (`crate::builtin_plugins::input::KeyboardSubsystem`); this plugin only
//! owns the `Focusable` *widget*. The `FocusManager` + `Focusable` trait +
//! focus/blur event payloads (`BlurEvent` / `FocusEvent` / `FocusChange`)
//! live in `crate::core::focus` (engine contract types).

pub(in crate::builtin_plugins) mod focusable;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Install the focus widget plugin (`Focusable` / `requestFocus`). Returns
/// the JS factory fns to be merged into `tur:std` by the orchestrator.
pub fn install_focus(_ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    Ok(focusable::bridge::fns())
}
