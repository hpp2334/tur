//! Unified JS <-> Rust value conversion traits.
//!
//! Every value that crosses the JS<->Rust boundary goes through [`FromJs`]
//! (decode, context-free) or [`IntoJs`] (encode, context-bearing). This replaces
//! the former `PropValue` / `FromBoaJsValue` / `IntoBoaJsValue` / `parse_*` /
//! `extract_*` menagerie with a single naming convention.
//!
//! ## Why `FromJs` takes no [`Context`]
//!
//! Layout and paint must be able to resolve reactive atoms (and thus decode
//! `Val<T>`) *without* touching the JS runtime, which is already borrowed at
//! that point. Decoding a materialized [`JsValue`] never needs a [`Context`]:
//! primitives read off the variant tag, enums are numbers, and complex types
//! (Color, Brush, reactive handles) are stored as boa `NativeObject` opaques
//! that downcast without the runtime. [`IntoJs`] *does* take a [`Context`]
//! because encoding may allocate JS objects/opaques.

use crate::core::layout::{
    Alignment, Axis, BorderPosition, BoxFit, ClipBehavior, CrossAxisAlignment, FlexDirection,
    FlexFit, HitTestBehavior, MainAxisAlignment, MainAxisSize, StackFit,
};
use crate::core::platform::Cursor;
use boa_engine::{Context, JsError, JsNativeError, JsValue};
use num_traits::FromPrimitive;

/// Decode a Rust value from a [`JsValue`] WITHOUT a boa [`Context`].
///
/// Returns `Err(JsError)` on a type mismatch. Callers that hold a [`Context`]
/// and want to surface a JS exception propagate with `?` (the error converts
/// into [`boa_engine::JsResult`]'s `Err` variant); silent callers use
/// [`.ok()`](Result::ok) / [`unwrap_or`](Result::unwrap_or).
pub trait FromJs: Sized {
    fn from_js(v: &JsValue) -> Result<Self, JsError>;
}

/// Encode a Rust value as a [`JsValue`]. May allocate, so it takes a [`Context`].
pub trait IntoJs {
    fn into_js(self, ctx: &mut Context) -> JsValue;
}

/// Convert an event payload into its JS callback arguments (one or more
/// positional [`JsValue`]s). Object-safe so the mutation queue can store
/// `Box<dyn IntoJsArgs>`. Implementations live alongside their event structs
/// in each event's owning module.
pub trait IntoJsArgs: 'static {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue>;
}

/// No-arg callbacks (lifecycle hooks: onMounted / onUpdated / beforeDestroy).
impl IntoJsArgs for () {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

/// Build a `TypeError`-flavored [`JsError`] describing the expected shape.
/// Used by [`FromJs`] impls; constructible without a [`Context`].
pub fn type_error(expected: &str) -> JsError {
    JsError::from(JsNativeError::typ().with_message(format!("expected {expected}")))
}

// --- primitive impls ---

impl FromJs for f64 {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_number().ok_or_else(|| type_error("a number"))
    }
}

impl FromJs for f32 {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_number()
            .map(|n| n as f32)
            .ok_or_else(|| type_error("a number"))
    }
}

impl FromJs for u32 {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_number()
            .map(|n| n as u32)
            .ok_or_else(|| type_error("a number"))
    }
}

impl FromJs for u64 {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_number()
            .map(|n| n as u64)
            .ok_or_else(|| type_error("a number"))
    }
}

impl FromJs for i32 {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_number()
            .map(|n| n as i32)
            .ok_or_else(|| type_error("a number"))
    }
}

impl FromJs for bool {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_boolean().ok_or_else(|| type_error("a boolean"))
    }
}

impl FromJs for String {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        v.as_string()
            .map(|s| s.to_std_string_escaped())
            .ok_or_else(|| type_error("a string"))
    }
}

// --- Cursor: decoded from a CSS keyword string (unrecognized keyword -> Err) ---

impl FromJs for Cursor {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        let s = v
            .as_string()
            .ok_or_else(|| type_error("a cursor keyword string"))?;
        Cursor::from_keyword(&s.to_std_string_escaped())
            .ok_or_else(|| type_error("a recognized cursor keyword"))
    }
}

// --- JsValue: identity decode. Lets an element hold a `Val<JsValue>` for props
// whose runtime value is a plain JS object that can only be decoded WITH a
// `Context` (e.g. a `{ x, y }` offset). The element resolves the `Val` to a
// `JsValue` via `read_val`, then field-reads with the layout JS face. ---
impl FromJs for JsValue {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        Ok(v.clone())
    }
}

// --- enums (stored as JS numbers, decoded via FromPrimitive) ---

macro_rules! impl_from_js_enum {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromJs for $ty {
                fn from_js(v: &JsValue) -> Result<Self, JsError> {
                    v.as_number()
                        .and_then(|n| <$ty as FromPrimitive>::from_i64(n as i64))
                        .ok_or_else(|| type_error(concat!("a ", stringify!($ty), " enum value")))
                }
            }
        )*
    };
}

impl_from_js_enum!(
    Alignment,
    Axis,
    BorderPosition,
    ClipBehavior,
    BoxFit,
    CrossAxisAlignment,
    FlexDirection,
    FlexFit,
    HitTestBehavior,
    MainAxisAlignment,
    MainAxisSize,
    StackFit,
);
