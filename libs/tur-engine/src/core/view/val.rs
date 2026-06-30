use std::collections::HashSet;

use boa_engine::JsValue;
use num_traits::FromPrimitive;
use tur_shared::{
    Alignment, Axis, BorderPosition, BoxFit, CrossAxisAlignment, Cursor, FlexDirection, FlexFit,
    HitTestBehavior, MainAxisAlignment, MainAxisSize, StackFit,
};

use crate::core::reactive::{extract_readable, AtomId, Readable};

// ---------------------------------------------------------------------------
// PropValue — trait for types that can be decoded from a JsValue WITHOUT a
// boa Context.  This is the key constraint: layout and paint must be able to
// resolve reactive atoms without touching the JS runtime.
//
// Primitive / enum types read directly off the JsValue variant tag.
// Complex types (Color, Brush) are stored as boa NativeObject opaques; their
// `PropValue` impls live in `core/bridge/color.rs` next to the opaque defs.
// ---------------------------------------------------------------------------

pub trait PropValue: Clone + 'static {
    fn from_js(v: &JsValue) -> Option<Self>;
}

// --- primitives ---

impl PropValue for f64 {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_number()
    }
}

impl PropValue for f32 {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_number().map(|n| n as f32)
    }
}

impl PropValue for u32 {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_number().map(|n| n as u32)
    }
}

impl PropValue for u64 {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_number().map(|n| n as u64)
    }
}

impl PropValue for i32 {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_number().map(|n| n as i32)
    }
}

impl PropValue for bool {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_boolean()
    }
}

impl PropValue for String {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_string()
            .map(|s| s.to_std_string_escaped())
    }
}

// --- Cursor: decoded from a CSS keyword string (unknown keyword → Default) ---

impl PropValue for Cursor {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_string()
            .map(|s| Cursor::from_keyword(&s.to_std_string_escaped()).unwrap_or_default())
    }
}

// --- enums (stored as JS numbers, decoded via FromPrimitive) ---

macro_rules! impl_prop_value_enum {
    ($($ty:ty),* $(,)?) => {
        $(
            impl PropValue for $ty {
                fn from_js(v: &JsValue) -> Option<Self> {
                    v.as_number()
                        .and_then(|n| <$ty as FromPrimitive>::from_i64(n as i64))
                }
            }
        )*
    };
}

impl_prop_value_enum!(
    Alignment,
    Axis,
    BorderPosition,
    BoxFit,
    CrossAxisAlignment,
    FlexDirection,
    FlexFit,
    HitTestBehavior,
    MainAxisAlignment,
    MainAxisSize,
    StackFit,
);

// ---------------------------------------------------------------------------
// Readable — typed read-only handle to a reactive atom (Source or Derived).
//
// Carries no phantom type parameter; the `T` of `Val<T>` provides type safety
// at the decode boundary (`PropValue::from_js`).  Object-valued atoms (e.g. a
// `Readable` holding a `TextEditingController`) are resolved via a raw
// `JsValue` read + downcast rather than `PropValue::from_js`.
// ---------------------------------------------------------------------------

impl<T> Readable<T> {
    pub fn is_dirty(&self, dirties: &HashSet<AtomId>) -> bool {
        dirties.contains(&self.id())
    }
}

// ---------------------------------------------------------------------------
// Val<T> — reactive-or-static value of a known Rust type.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Val<T: PropValue> {
    Static(T),
    Reactive(Readable<T>),
}

impl<T: PropValue> Val<T> {
    /// Returns the atom id if this is a reactive val.
    pub fn atom(&self) -> Option<AtomId> {
        match self {
            Val::Reactive(r) => Some(r.id()),
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
    pub fn is_dirty(&self, dirties: &HashSet<AtomId>) -> bool {
        match self {
            Val::Reactive(r) => dirties.contains(&r.id()),
            _ => false,
        }
    }
}

/// Interpret a JS prop value as a `Val<T>`: if it's an atom handle, wrap as
/// `Reactive`; otherwise try to parse it as `T` via `PropValue::from_js`.
///
/// Returns `None` for undefined/null or when the value can't be decoded.
pub fn val_from_js<T: PropValue>(v: &JsValue) -> Option<Val<T>> {
    if v.is_undefined() || v.is_null() {
        return None;
    }
    match extract_readable::<T>(v) {
        Some(readable) => Some(Val::Reactive(readable)),
        None => T::from_js(v).map(Val::Static),
    }
}
