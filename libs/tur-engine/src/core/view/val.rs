use std::collections::HashSet;

use boa_engine::JsValue;

use crate::core::js_value::FromJs;
use crate::core::reactive::{AnyReadable, Readable};

// ---------------------------------------------------------------------------
// Val<T> — reactive-or-static value of a known Rust type.
//
// `T` must be [`FromJs`] (context-free decode) plus `Clone + 'static` (so the
// `Val` itself can be `Clone`). Reactive resolution reads the atom's current
// `JsValue` and decodes via `T::from_js` during layout/paint, which is why the
// decode cannot depend on a boa `Context`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Val<T: FromJs + Clone + 'static> {
    Static(T),
    Reactive(Readable<T>),
}

impl<T: FromJs + Clone + 'static> Val<T> {
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
/// `Reactive`; otherwise try to decode it as `T` via [`FromJs::from_js`].
///
/// Returns `None` for undefined/null or when the value can't be decoded.
pub fn val_from_js<T: FromJs + Clone + 'static>(v: &JsValue) -> Option<Val<T>> {
    if v.is_undefined() || v.is_null() {
        return None;
    }
    match Readable::<T>::from_js(v) {
        Ok(readable) => Some(Val::Reactive(readable)),
        Err(_) => T::from_js(v).ok().map(Val::Static),
    }
}
