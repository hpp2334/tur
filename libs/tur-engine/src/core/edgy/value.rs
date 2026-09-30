//! `Value` — the reactive substrate's native value type.
//!
//! The store KV (`StoreKv` slots) and source seeds hold `Value`, NOT
//! [`JsValue`]: the substrate is native-first, so a realm-free instance (the
//! rut rail) can mint, write, and read structured atoms without a JS realm.
//! Every atom shape the JS rail produces crosses into `Value` at the JS
//! boundary ([`Value::from_js`]) and back out where JS reads
//! ([`Value::to_js`]) — the store itself never sees a `JsValue`.
//!
//! The set is closed over the shapes the engine actually stores: scalars,
//! strings, byte blobs, lists (JS arrays), string-keyed maps (JS plain
//! objects), and — the one JS-interop escape hatch — [`Value::Opaque`],
//! which wraps any non-plain-data `JsValue` (engine opaques like
//! `Color`/`Brush`/controller handles, class instances, functions) by
//! identity. Wrapping (rather than rejecting) keeps the JS rail
//! behavior-identical: an opaque round-trips to the very same JS object, so
//! identity-sensitive JS code (e.g. the implicit-animation retarget probe)
//! sees no change.

use std::collections::BTreeMap;
use std::rc::Rc;

use boa_engine::object::builtins::{AlignedVec, JsArray, JsArrayBuffer};
use boa_engine::value::JsVariant;
use boa_engine::{Context, JsError, JsNativeError, JsValue, js_string};

use crate::core::layout::{
    Alignment, Axis, BorderPosition, BoxFit, ClipBehavior, CrossAxisAlignment, FlexDirection,
    FlexFit, HitTestBehavior, MainAxisAlignment, MainAxisSize, StackFit,
};
use crate::core::shell::Cursor;
use num_traits::FromPrimitive;

// ---------------------------------------------------------------------------
// Value — the closed native value set.
// ---------------------------------------------------------------------------

/// A native reactive value. Cheap to clone (all payloads are `Copy` or `Rc`).
///
/// Equality mirrors JS semantics: primitives compare by value; `List` /
/// `Map` / `Opaque` compare by reference identity (a fresh structurally
/// equal list is a NEW value, like a fresh JS array).
#[derive(Debug, Clone, Default)]
pub enum Value {
    /// `undefined` / `null` (the substrate does not distinguish them; the JS
    /// boundary maps both here and `to_js` emits `undefined`).
    #[default]
    Nil,
    Bool(bool),
    Num(f64),
    Str(Rc<str>),
    /// Raw bytes (the rut rail's blob shape; no JS-rail producer today —
    /// `to_js` materializes an `ArrayBuffer`).
    Bytes(Rc<[u8]>),
    /// An ordered list (JS arrays, rut lists).
    List(Rc<Vec<Value>>),
    /// A string-keyed map (JS plain objects, rut records). `BTreeMap` keeps
    /// iteration deterministic (sorted by key) for dev tools + round-trips.
    Map(Rc<BTreeMap<Rc<str>, Value>>),
    /// A non-plain-data JS value held by identity (engine opaques, class
    /// instances, functions). The JS-interop escape hatch — see the module
    /// docs. Never inspected by the substrate.
    Opaque(Rc<JsValue>),
}

impl Value {
    /// A `Str` from anything string-like.
    pub fn str(s: impl Into<Rc<str>>) -> Value {
        Value::Str(s.into())
    }

    /// A `List` from an iterator of values.
    pub fn list(items: impl IntoIterator<Item = Value>) -> Value {
        Value::List(Rc::new(items.into_iter().collect()))
    }

    /// A `Map` from an iterator of key/value pairs.
    pub fn map(entries: impl IntoIterator<Item = (impl Into<Rc<str>>, Value)>) -> Value {
        Value::Map(Rc::new(
            entries.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        ))
    }

    /// Wrap an arbitrary JS value opaquely (identity-preserving).
    pub fn opaque(v: &JsValue) -> Value {
        Value::Opaque(Rc::new(v.clone()))
    }

    // ----- accessors --------------------------------------------------------

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<Rc<str>, Value>> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// The inner JS value of an [`Value::Opaque`], if this is one.
    pub fn as_opaque(&self) -> Option<&JsValue> {
        match self {
            Value::Opaque(v) => Some(v),
            _ => None,
        }
    }

    /// List item by index (lists only).
    pub fn at(&self, index: usize) -> Option<&Value> {
        self.as_list().and_then(|items| items.get(index))
    }

    /// Map entry by key (maps only).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map().and_then(|m| m.get(key))
    }

    // ----- JS boundary ------------------------------------------------------
    //
    // The JsValue ↔ Value conversions are TOTAL: every `JsValue` maps into a
    // `Value` (plain data decodes; everything else wraps opaquely), and every
    // `Value` maps back to a `JsValue`. Only the object walk can hit exotic
    // property shapes — on any error the object is wrapped opaquely, so
    // `from_js` itself never fails.

    /// Convert a JS value into its native `Value` representation (total).
    ///
    /// Plain data decodes structurally: arrays → [`Value::List`], ordinary
    /// objects (no Rust `JsData` payload — JS `{}` literals, `Object.create`)
    /// → [`Value::Map`] over their own string-keyed properties. Everything
    /// else — engine opaques (`Color` handles, controllers, view handles),
    /// class instances, functions, symbols — wraps as [`Value::Opaque`] with
    /// identity preserved.
    pub fn from_js(v: &JsValue, ctx: &mut Context) -> Value {
        match v.variant() {
            JsVariant::Undefined | JsVariant::Null => Value::Nil,
            JsVariant::Boolean(b) => Value::Bool(b),
            JsVariant::Float64(n) => Value::Num(n),
            JsVariant::Integer32(i) => Value::Num(f64::from(i)),
            JsVariant::String(s) => Value::str(s.to_std_string_escaped()),
            JsVariant::BigInt(_) | JsVariant::Symbol(_) => Value::opaque(v),
            JsVariant::Object(obj) => Self::object_from_js(&obj, ctx),
        }
    }

    /// Decode a JS object: arrays → List, ordinary objects (no Rust
    /// `JsData` payload) → Map, everything else → opaque. The walk degrades
    /// to opaque on any property error, so exotic objects (throwing getters,
    /// revoked proxies) round-trip by identity instead of failing the
    /// boundary.
    fn object_from_js(obj: &boa_engine::JsObject, ctx: &mut Context) -> Value {
        // Arrays → List (indexed elements in order).
        if obj.is_array() {
            let arr = match JsArray::from_object(obj.clone()) {
                Ok(arr) => arr,
                Err(_) => return Value::opaque(&obj.clone().into()),
            };
            let Ok(len) = arr.length(ctx) else {
                return Value::opaque(&obj.clone().into());
            };
            let mut items = Vec::with_capacity(len as usize);
            for i in 0..len {
                match arr.at(i as i64, ctx) {
                    Ok(item) => items.push(Value::from_js(&item, ctx)),
                    Err(_) => return Value::opaque(&obj.clone().into()),
                }
            }
            return Value::List(Rc::new(items));
        }

        // Plain data objects (no Rust `JsData` payload AND the standard
        // object prototype — `{}` literals, `Object.create(null)`) → Map
        // over their own string-keyed properties. Both other shapes keep
        // their identity opaquely: engine opaques (`Color` handles,
        // controllers, view handles — any `NativeObject`) and user class
        // instances (a non-`Object.prototype` chain makes them non-plain
        // even though their data cell is ordinary).
        let object_proto = ctx.intrinsics().constructors().object().prototype();
        let is_plain_data = obj.is_ordinary()
            && match obj.prototype() {
                None => true,
                Some(proto) => proto == object_proto,
            };
        if !is_plain_data {
            return Value::opaque(&obj.clone().into());
        }

        let keys = match obj.own_property_keys(ctx) {
            Ok(keys) => keys,
            Err(_) => return Value::opaque(&obj.clone().into()),
        };
        let mut entries = BTreeMap::new();
        for key in keys {
            // String keys only: index keys belong to the array shape, symbol
            // keys are not plain data.
            let boa_engine::property::PropertyKey::String(key_str) = key else {
                continue;
            };
            let value = match obj.get(key_str.clone(), ctx) {
                Ok(value) => value,
                Err(_) => return Value::opaque(&obj.clone().into()),
            };
            entries.insert(
                Rc::from(key_str.to_std_string_escaped().as_str()),
                Value::from_js(&value, ctx),
            );
        }
        Value::Map(Rc::new(entries))
    }

    /// Convert this `Value` back into a JS value (total over the shapes the
    /// engine stores). Scalars/strings map directly; lists become arrays;
    /// maps become plain objects; opaques re-materialize their held object
    /// (identity preserved); bytes become an `ArrayBuffer`.
    pub fn to_js(&self, ctx: &mut Context) -> JsValue {
        match self {
            Value::Nil => JsValue::undefined(),
            Value::Bool(b) => JsValue::from(*b),
            Value::Num(n) => JsValue::from(*n),
            Value::Str(s) => JsValue::from(js_string!(s.as_ref())),
            Value::Bytes(bytes) => JsArrayBuffer::from_byte_block(
                AlignedVec::from_iter(0, bytes.iter().copied()),
                ctx,
            )
            .map(Into::into)
            .unwrap_or_else(|_| JsValue::undefined()),
            Value::List(items) => {
                let js_items: Vec<JsValue> = items.iter().map(|item| item.to_js(ctx)).collect();
                JsArray::from_iter(js_items, ctx).into()
            }
            Value::Map(entries) => {
                let obj = boa_engine::JsObject::with_object_proto(ctx.intrinsics());
                for (key, value) in entries.iter() {
                    let _ =
                        obj.create_data_property(js_string!(key.as_ref()), value.to_js(ctx), ctx);
                }
                obj.into()
            }
            Value::Opaque(v) => v.as_ref().clone(),
        }
    }
}

/// Value equality mirrors JS: primitives by value, containers/opaque by
/// reference identity (`Rc::ptr_eq` / `JsValue` object identity).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b, // NaN != NaN (JS semantics)
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b),
            (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b),
            (Value::Opaque(a), Value::Opaque(b)) => a == b,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// FromValue — decode a Rust value from a `Value` (context-free).
//
// The native twin of [`crate::core::js_runtime::js_value::FromJs`]: `Val<T>`
// resolves reactive atoms to `Value` and decodes via this trait, so
// layout/paint never touch a `JsValue`. `FromJs` remains the JS-bridge
// decode trait (atom handles, args, direct prop reads).
// ---------------------------------------------------------------------------

pub trait FromValue: Sized {
    fn from_value(v: &Value) -> Result<Self, JsError>;
}

/// Build a `TypeError`-flavored error describing the expected shape (the
/// `FromValue` twin of `js_value::type_error`).
pub fn type_error(expected: &str) -> JsError {
    JsError::from(JsNativeError::typ().with_message(format!("expected {expected}")))
}

// --- primitives ---

macro_rules! impl_from_value_float {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromValue for $ty {
                fn from_value(v: &Value) -> Result<Self, JsError> {
                    v.as_num().map(|n| n as $ty).ok_or_else(|| type_error("a number"))
                }
            }
        )*
    };
}

impl_from_value_float!(f64, f32);

macro_rules! impl_from_value_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromValue for $ty {
                fn from_value(v: &Value) -> Result<Self, JsError> {
                    v.as_num()
                        .map(|n| n as $ty)
                        .ok_or_else(|| type_error("a number"))
                }
            }
        )*
    };
}

impl_from_value_int!(u32, u64, i32, usize);

impl FromValue for bool {
    fn from_value(v: &Value) -> Result<Self, JsError> {
        v.as_bool().ok_or_else(|| type_error("a boolean"))
    }
}

impl FromValue for String {
    fn from_value(v: &Value) -> Result<Self, JsError> {
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| type_error("a string"))
    }
}

/// Identity decode — a prop held raw as a native `Value` (the realm-free
/// twin of the old `Val<JsValue>`; plain-data objects are field-read from
/// the `Map` without a realm).
impl FromValue for Value {
    fn from_value(v: &Value) -> Result<Self, JsError> {
        Ok(v.clone())
    }
}

// --- Cursor: decoded from a CSS keyword string ---

impl FromValue for Cursor {
    fn from_value(v: &Value) -> Result<Self, JsError> {
        let s = v
            .as_str()
            .ok_or_else(|| type_error("a cursor keyword string"))?;
        Cursor::from_keyword(s).ok_or_else(|| type_error("a recognized cursor keyword"))
    }
}

// --- enums (stored as numbers, decoded via FromPrimitive) ---

macro_rules! impl_from_value_enum {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromValue for $ty {
                fn from_value(v: &Value) -> Result<Self, JsError> {
                    v.as_num()
                        .and_then(|n| <$ty as FromPrimitive>::from_i64(n as i64))
                        .ok_or_else(|| type_error(concat!("a ", stringify!($ty), " enum value")))
                }
            }
        )*
    };
}

impl_from_value_enum!(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context::builder().build().expect("test realm")
    }

    #[test]
    fn scalars_round_trip_through_js() {
        let mut cx = ctx();
        for v in [
            JsValue::undefined(),
            JsValue::from(true),
            JsValue::from(42.5),
            JsValue::from(js_string!("hello")),
        ] {
            let native = Value::from_js(&v, &mut cx);
            let back = native.to_js(&mut cx);
            assert!(
                back.strict_equals(&v),
                "{v:?} should round-trip (got {back:?})"
            );
        }
        // null collapses into Nil by design — the substrate does not
        // distinguish the two empties, and `to_js` emits `undefined`.
        assert!(
            Value::from_js(&JsValue::null(), &mut cx)
                .to_js(&mut cx)
                .strict_equals(&JsValue::undefined())
        );
    }

    #[test]
    fn plain_objects_decode_to_maps_and_back() {
        let mut cx = ctx();
        let obj = boa_engine::JsObject::with_object_proto(cx.intrinsics());
        obj.create_data_property(js_string!("width"), JsValue::from(800.0), &mut cx)
            .unwrap();
        obj.create_data_property(js_string!("label"), JsValue::from(js_string!("hi")), &mut cx)
            .unwrap();

        let native = Value::from_js(&obj.clone().into(), &mut cx);
        let map = native.as_map().expect("plain object decodes to a Map");
        assert_eq!(map.get("width").and_then(Value::as_num), Some(800.0));
        assert_eq!(map.get("label").and_then(Value::as_str), Some("hi"));

        // Round-trip: the JS value is again a plain object with the fields.
        let back = native.to_js(&mut cx);
        let back_obj = back.as_object().expect("map re-materializes as an object");
        let width = back_obj.get(js_string!("width"), &mut cx).unwrap();
        assert_eq!(width.as_number(), Some(800.0));
    }

    #[test]
    fn arrays_decode_to_lists_and_back() {
        let mut cx = ctx();
        let arr = JsArray::from_iter(
            [JsValue::from(1.0), JsValue::from(js_string!("two"))],
            &mut cx,
        );
        let native = Value::from_js(&arr.clone().into(), &mut cx);
        let items = native.as_list().expect("array decodes to a List");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_num(), Some(1.0));
        assert_eq!(items[1].as_str(), Some("two"));

        let back = native.to_js(&mut cx);
        assert!(
            back.as_object().unwrap().is_array(),
            "list re-materializes as a JS array"
        );
    }

    #[test]
    fn non_plain_values_wrap_opaque_with_identity() {
        let mut cx = ctx();
        // Functions wrap opaquely (never decode into a Map).
        let f = boa_engine::object::FunctionObjectBuilder::new(
            cx.realm(),
            boa_engine::native_function::NativeFunction::from_copy_closure(|_, _, _| {
                Ok(JsValue::undefined())
            }),
        )
        .build();
        let native = Value::from_js(&f.clone().into(), &mut cx);
        assert!(native.as_opaque().is_some(), "functions wrap opaquely");
        let back = native.to_js(&mut cx);
        // The opaque re-materializes the very same function object.
        let f_obj: boa_engine::JsObject = f.into();
        assert!(
            back.as_object().is_some_and(|o| o == f_obj),
            "opaque re-materializes the same object"
        );
    }

    #[test]
    fn native_data_objects_wrap_opaque_even_with_the_standard_proto() {
        use boa_gc::{Finalize, Trace};

        #[derive(Debug, Trace, Finalize, boa_engine::JsData)]
        #[boa_gc(unsafe_empty_trace)]
        struct Payload(u8);

        let mut cx = ctx();
        // An engine opaque carries Rust `JsData` on the STANDARD object
        // prototype (the engine's `wrap_opaque` shape) — it must wrap by
        // identity, never decode into an (empty) Map.
        let obj = boa_engine::JsObject::from_proto_and_data(
            cx.intrinsics().constructors().object().prototype(),
            Payload(7),
        );
        let native = Value::from_js(&obj.clone().into(), &mut cx);
        assert!(
            native.as_opaque().is_some(),
            "a JsData object is not plain data"
        );
        let back = native.to_js(&mut cx);
        assert_eq!(
            back.as_object(),
            Some(obj),
            "opaque re-materializes the same object"
        );
    }

    #[test]
    fn class_instances_wrap_opaque() {
        let mut cx = ctx();
        // A user class instance has a non-Object.prototype chain — not plain
        // data even though its data cell is ordinary. It wraps by identity.
        let result = cx
            .eval(boa_engine::Source::from_bytes(
                "class Point { constructor(x) { this.x = x; } } new Point(3)",
            ))
            .expect("instance eval");
        let native = Value::from_js(&result, &mut cx);
        assert!(
            native.as_opaque().is_some(),
            "a class instance is not plain data"
        );
    }

    #[test]
    fn equality_is_js_semantics() {
        let a = Value::list([Value::Num(1.0)]);
        let b = Value::list([Value::Num(1.0)]);
        assert_eq!(a, a.clone(), "the same list equals itself");
        assert_ne!(a, b, "fresh structurally-equal lists differ (JS identity)");

        assert_eq!(Value::Num(1.5), Value::Num(1.5));
        assert_eq!(Value::str("s"), Value::str("s"));
        assert_ne!(Value::Num(1.0), Value::Str("1.0".into()));
    }

    #[test]
    fn nan_is_not_equal_to_nan() {
        assert_ne!(Value::Num(f64::NAN), Value::Num(f64::NAN));
    }

    #[test]
    fn from_value_decodes_scalars() {
        assert_eq!(f64::from_value(&Value::Num(3.5)).unwrap(), 3.5);
        assert_eq!(u32::from_value(&Value::Num(7.0)).unwrap(), 7);
        assert!(bool::from_value(&Value::Bool(true)).unwrap());
        assert_eq!(String::from_value(&Value::str("s")).unwrap(), "s");
        assert!(f64::from_value(&Value::Nil).is_err(), "Nil is not a number");
        assert!(
            bool::from_value(&Value::Num(1.0)).is_err(),
            "Num is not a bool"
        );
    }
}
