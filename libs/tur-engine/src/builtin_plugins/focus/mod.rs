//! Focus widget plugin:
//! - `Focusable` element.
//!
//! Key-event dispatch and bubble-up live in the sibling `input` plugin
//! (`crate::builtin_plugins::input::KeyboardSubsystem`); this plugin only
//! owns the `Focusable` *widget*. The `FocusManager` + `Focusable` trait +
//! focus/blur event payloads (`BlurEvent` / `FocusEvent` / `FocusChange`)
//! live in `crate::core::focus` (engine contract types).

pub(in crate::builtin_plugins) mod focusable;

pub use focusable::FocusableView;
