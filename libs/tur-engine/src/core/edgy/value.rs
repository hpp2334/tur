//! `Value` — the reactive substrate's native value type.
//!
//! The store KV (`StoreKv` slots) and source seeds hold `Value`: the
//! substrate is native — the rut rail mints, writes, and reads structured
//! atoms without any script realm.
//!
//! The set is closed over the shapes the engine actually stores: scalars,
//! strings, byte blobs, lists (rut lists), string-keyed maps (rut records),
//! and — the one escape hatch — [`Value::Opaque`], which wraps any
//! non-plain-data host value (engine opaques like `Color`/`Brush` handles)
//! by identity. Wrapping (rather than rejecting) keeps reactive props that
//! carry opaque handles behavior-identical: an opaque round-trips to the
//! very same handle, so identity-sensitive code (e.g. the implicit-animation
//! retarget probe) sees no change.

use std::collections::BTreeMap;
use std::rc::Rc;

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
    /// Absent / unset.
    #[default]
    Nil,
    Bool(bool),
    Num(f64),
    Str(Rc<str>),
    /// Raw bytes (net bodies, encoded text).
    Bytes(Rc<[u8]>),
    /// An ordered list (rut lists).
    List(Rc<Vec<Value>>),
    /// A string-keyed map (rut records). `BTreeMap` keeps iteration
    /// deterministic (sorted by key) for dev tools + round-trips.
    Map(Rc<BTreeMap<Rc<str>, Value>>),
    /// A non-plain-data host value held by identity (engine opaques like
    /// `Color`/`Brush`/controller handles). The escape hatch — see the
    /// module docs. Never inspected by the substrate.
    Opaque(Rc<dyn std::any::Any>),
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

    /// Wrap an arbitrary host value opaquely (identity-preserving).
    pub fn opaque(v: Rc<dyn std::any::Any>) -> Value {
        Value::Opaque(v)
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

    /// The inner host value of a [`Value::Opaque`], if this is one.
    pub fn as_opaque(&self) -> Option<&Rc<dyn std::any::Any>> {
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
}

/// Value equality: primitives by value, containers/opaque by reference
/// identity (`Rc::ptr_eq` — a fresh list is a NEW value).
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
            (Value::Opaque(a), Value::Opaque(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// FromValue — decode a Rust value from a `Value` (context-free).
//
// `Val<T>` resolves reactive atoms to `Value` and decodes via this trait,
// so layout/paint never depend on any script runtime.
// ---------------------------------------------------------------------------

pub trait FromValue: Sized {
    fn from_value(v: &Value) -> Result<Self, String>;
}

/// Build the decode-error message describing the expected shape.
pub fn type_error(expected: &str) -> String {
    format!("expected {expected}")
}

// --- primitives ---

macro_rules! impl_from_value_float {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromValue for $ty {
                fn from_value(v: &Value) -> Result<Self, String> {
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
                fn from_value(v: &Value) -> Result<Self, String> {
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
    fn from_value(v: &Value) -> Result<Self, String> {
        v.as_bool().ok_or_else(|| type_error("a boolean"))
    }
}

impl FromValue for String {
    fn from_value(v: &Value) -> Result<Self, String> {
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| type_error("a string"))
    }
}

/// Identity decode — a prop held raw as a native `Value`.
impl FromValue for Value {
    fn from_value(v: &Value) -> Result<Self, String> {
        Ok(v.clone())
    }
}

// --- Cursor: decoded from a CSS keyword string ---

impl FromValue for Cursor {
    fn from_value(v: &Value) -> Result<Self, String> {
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
                fn from_value(v: &Value) -> Result<Self, String> {
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
