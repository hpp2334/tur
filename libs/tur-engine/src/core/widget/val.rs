use std::collections::HashSet;
use std::marker::PhantomData;

use boa_engine::JsValue;
use boa_gc::{Finalize, Trace};
use num_traits::FromPrimitive;
use tur_shared::{
    Alignment, Axis, BorderPosition, BoxFit, Brush, Color, CrossAxisAlignment, FlexDirection,
    FlexFit, HitTestBehavior, MainAxisAlignment, MainAxisSize, StackFit,
};

use crate::core::reactive::{extract_atom, AtomId};

// ---------------------------------------------------------------------------
// Opaque wrappers for tur-shared types so they can be stored inside boa
// JS objects (NativeObject).  tur-shared cannot depend on boa, so we wrap
// here.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ColorOpaque(pub Color);

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct BrushOpaque(pub Brush);

// ---------------------------------------------------------------------------
// PropValue — trait for types that can be decoded from a JsValue WITHOUT a
// boa Context.  This is the key constraint: layout and paint must be able to
// resolve reactive atoms without touching the JS runtime.
//
// Primitive / enum types read directly off the JsValue variant tag.
// Complex types (Color, Brush) are stored as boa NativeObject opaques so a
// simple `downcast_ref` suffices — no property access, no Context.
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

// --- opaque types (stored as boa NativeObject) ---

impl PropValue for Color {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_object()?.downcast_ref::<ColorOpaque>().map(|c| c.0)
    }
}

impl PropValue for Brush {
    fn from_js(v: &JsValue) -> Option<Self> {
        let obj = v.as_object()?;
        if let Some(b) = obj.downcast_ref::<BrushOpaque>() {
            return Some(b.0.clone());
        }
        // A Color is implicitly a solid Brush.
        if let Some(c) = obj.downcast_ref::<ColorOpaque>() {
            return Some(Brush::SolidColor(c.0));
        }
        None
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
// ReadableAtom<T> — typed read-only handle to a reactive atom.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ReadableAtom<T: PropValue> {
    pub(crate) id: AtomId,
    _marker: PhantomData<T>,
}

impl<T: PropValue> ReadableAtom<T> {
    pub fn new(id: AtomId) -> Self {
        ReadableAtom {
            id,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> AtomId {
        self.id
    }

    pub fn is_dirty(&self, dirties: &HashSet<AtomId>) -> bool {
        dirties.contains(&self.id)
    }
}

// ---------------------------------------------------------------------------
// Val<T> — reactive-or-static value of a known Rust type.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Val<T: PropValue> {
    Static(T),
    Reactive(ReadableAtom<T>),
}

impl<T: PropValue> Val<T> {
    /// Returns the atom id if this is a reactive val.
    pub fn atom(&self) -> Option<AtomId> {
        match self {
            Val::Reactive(a) => Some(a.id),
            _ => None,
        }
    }

    /// Returns `true` if this is a reactive val whose atom is in `dirties`.
    pub fn is_dirty(&self, dirties: &HashSet<AtomId>) -> bool {
        match self {
            Val::Reactive(a) => dirties.contains(&a.id),
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
    match extract_atom(v) {
        Some(id) => Some(Val::Reactive(ReadableAtom::new(id))),
        None => T::from_js(v).map(Val::Static),
    }
}
