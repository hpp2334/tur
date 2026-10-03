use std::collections::HashSet;

use crate::core::edgy::reactive::{AnyReadable, Readable};
use crate::core::edgy::value::FromValue;

// ---------------------------------------------------------------------------
// Val<T> — reactive-or-static value of a known Rust type.
//
// `T` must be [`FromValue`] (context-free decode off the native `Value`) plus
// `Clone + 'static` (so the `Val` itself can be `Clone`). Reactive resolution
// reads the atom's current native value and decodes via `T::from_value`
// during layout/paint — the store holds native values only.
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
