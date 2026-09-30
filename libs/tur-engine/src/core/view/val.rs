use std::collections::HashSet;

use boa_engine::{Context, JsValue};

use crate::core::edgy::reactive::{AnyReadable, Readable};
use crate::core::edgy::value::{FromValue, Value};
use crate::core::js_runtime::js_value::FromJs;

// ---------------------------------------------------------------------------
// Val<T> — reactive-or-static value of a known Rust type.
//
// `T` must be [`FromValue`] (context-free decode off the native `Value`) plus
// `Clone + 'static` (so the `Val` itself can be `Clone`). Reactive resolution
// reads the atom's current native value and decodes via `T::from_value`
// during layout/paint — the store holds no `JsValue`, so the decode never
// touches a JS realm.
//
// The STATIC path still starts at a `JsValue` (element props arrive from a
// JS factory): [`val_from_js`] checks the atom-handle shape first (via
// [`FromJs`], the JS-bridge decode), then converts the value into a native
// [`Value`] and decodes via `T::from_value`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Val<T: FromValue + Clone + 'static> {
    Static(T),
    Reactive(Readable<T>),
}

impl<T: FromValue + Clone + 'static> Val<T> {
    /// Returns the atom as an erased handle if this is a reactive val.
    pub fn atom(&self) -> Option<AnyReadable> {
        match self {
            Val::Reactive(r) => Some(r.to_any()),
            _ => None,
        }
    }

    /// Returns the static value if this is a `Val::Static`.
    pub fn as_static(&self) -> Option<&T> {
        match self {
            Val::Static(v) => Some(v),
            _ => None,
        }
    }

    /// Returns `true` if this is a reactive val whose atom is in `dirties`.
    pub fn is_dirty(&self, dirties: &HashSet<AnyReadable>) -> bool {
        match self {
            Val::Reactive(r) => dirties.contains(&r.to_any()),
            _ => false,
        }
    }
}

impl<T> Readable<T> {
    pub fn is_dirty(&self, dirties: &HashSet<AnyReadable>) -> bool {
        dirties.contains(&self.to_any())
    }
}

/// Interpret a JS prop value as a `Val<T>`: if it's an atom handle, wrap as
/// `Reactive`; otherwise convert it to the native [`Value`] shape and decode
/// as `T` via [`FromValue::from_value`].
///
/// Returns `None` for undefined/null or when the value can't be decoded.
pub fn val_from_js<T: FromValue + Clone + 'static>(
    v: &JsValue,
    ctx: &mut Context,
) -> Option<Val<T>> {
    if v.is_undefined() || v.is_null() {
        return None;
    }
    if let Ok(readable) = Readable::<T>::from_js(v) {
        return Some(Val::Reactive(readable));
    }
    let native = Value::from_js(v, ctx);
    T::from_value(&native).ok().map(Val::Static)
}
