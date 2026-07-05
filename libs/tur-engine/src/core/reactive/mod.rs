use std::cell::RefCell;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{js_string, Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

mod store;

pub use store::Store;
pub use store::{ReactiveCore, ReactiveReadStore, ReactiveReadJsContext, SubscriberIndexStore};

/// Unique identifier for a reactive atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomId(pub u32);

impl AtomId {
    #[inline]
    pub fn new(id: u32) -> Self {
        AtomId(id)
    }

    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Opaque identifier for an external subscriber (e.g. an `NodeId`)
/// that reads a reactive atom during layout.  The store records atom→subscriber
/// edges so a reactive flush can mark affected subscribers dirty.  Kept as a
/// plain `u64` newtype so the reactive module stays decoupled from the element
/// module — callers convert `NodeId` → `SubscriberId` at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriberId(u64);

impl SubscriberId {
    #[inline]
    pub fn new(id: u64) -> Self {
        SubscriberId(id)
    }

    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Kind tag stored inside [`AtomHandle`] so the JS→Rust extraction boundary
/// can recover the typed handle.  The Store itself does **not** track kinds —
/// the Rust type system carries them via `Source`, `Derived`, `Mutation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomKind {
    Source,
    Derived,
    Mutation,
}

// ---------------------------------------------------------------------------
// Typed atom handles — the Rust type system encodes the atom kind.
//
// `T` is a phantom type parameter: it exists only for compile-time type
// safety.  The Store stores `JsValue` for all atoms; `T` is erased to
// `PhantomData<fn() -> T>` (covariant, no Send/Sync/'static overhead).
// ---------------------------------------------------------------------------

/// Handle for a source atom (writable, never stale).
pub struct Source<T>(pub AtomId, PhantomData<fn() -> T>);

/// Handle for a derived atom (lazy, recomputes on read when stale).
pub struct Derived<T>(pub AtomId, PhantomData<fn() -> T>);

impl<T> Source<T> {
    #[inline]
    pub fn id(&self) -> AtomId {
        self.0
    }

    pub fn from_id(id: AtomId) -> Self {
        Source(id, PhantomData)
    }
}

impl<T> Derived<T> {
    #[inline]
    pub fn id(&self) -> AtomId {
        self.0
    }

    pub fn from_id(id: AtomId) -> Self {
        Derived(id, PhantomData)
    }
}

// --- manual trait impls (no `T` bounds — phantom newtype pattern) ---

impl<T> Clone for Source<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Source<T> {}
impl<T> PartialEq for Source<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for Source<T> {}
impl<T> Hash for Source<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl<T> std::fmt::Debug for Source<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Source").field(&self.0).finish()
    }
}

impl<T> Clone for Derived<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Derived<T> {}
impl<T> PartialEq for Derived<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for Derived<T> {}
impl<T> Hash for Derived<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl<T> std::fmt::Debug for Derived<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Derived").field(&self.0).finish()
    }
}

/// Handle for a mutation atom (callable side-effect closure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mutation(pub AtomId);

/// Read-only reference to either a [`Source<T>`] or a [`Derived<T>`].  Used by
/// `Val<T>::Reactive`, `Store::read`, and the subscriber graph.
pub enum Readable<T> {
    Source(Source<T>),
    Derived(Derived<T>),
}

impl<T> Readable<T> {
    #[inline]
    pub fn id(&self) -> AtomId {
        match self {
            Readable::Source(s) => s.0,
            Readable::Derived(d) => d.0,
        }
    }
}

impl<T> Clone for Readable<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Readable<T> {}
impl<T> PartialEq for Readable<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl<T> Eq for Readable<T> {}
impl<T> Hash for Readable<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}
impl<T> std::fmt::Debug for Readable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Readable::Source(s) => f.debug_tuple("Readable::Source").field(&s.0).finish(),
            Readable::Derived(d) => f.debug_tuple("Readable::Derived").field(&d.0).finish(),
        }
    }
}

/// Untyped readable — carries a raw `JsValue` (e.g. a JS array or object)
/// that is not decoded via `PropValue`.
pub type AnyReadable = Readable<JsValue>;

impl<T> From<Source<T>> for Readable<T> {
    #[inline]
    fn from(s: Source<T>) -> Self {
        Readable::Source(s)
    }
}

impl<T> From<Derived<T>> for Readable<T> {
    #[inline]
    fn from(d: Derived<T>) -> Self {
        Readable::Derived(d)
    }
}

// ---------------------------------------------------------------------------
// AtomHandle — opaque JS wrapper carrying the kind tag.
// ---------------------------------------------------------------------------

/// Opaque handle returned to JS for an atom.  Carries an [`AtomKind`] tag so
/// the extraction boundary can produce the correct typed handle.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct AtomHandle {
    pub(crate) id: AtomId,
    pub(crate) kind: AtomKind,
}

impl AtomHandle {
    pub fn new(id: AtomId, kind: AtomKind) -> Self {
        AtomHandle { id, kind }
    }
}

/// Extract a [`Readable<T>`] (Source or Derived) from a JS value.  Returns
/// `None` for mutations or non-handle values.
pub fn extract_readable<T>(value: &JsValue) -> Option<Readable<T>> {
    let obj = value.as_object()?;
    let h = obj.downcast_ref::<AtomHandle>()?;
    match h.kind {
        AtomKind::Source => Some(Readable::Source(Source::from_id(h.id))),
        AtomKind::Derived => Some(Readable::Derived(Derived::from_id(h.id))),
        AtomKind::Mutation => None,
    }
}

/// Extract a [`Readable<T>`] or raise a TypeError.
pub fn require_readable<T>(args: &[JsValue], idx: usize) -> JsResult<Readable<T>> {
    extract_readable(args.get_or_undefined(idx)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("expected a source or derived atom handle"),
        )
    })
}

/// Extract a [`Mutation`] from a JS value.
pub fn extract_mutation(value: &JsValue) -> Option<Mutation> {
    let obj = value.as_object()?;
    let h = obj.downcast_ref::<AtomHandle>()?;
    match h.kind {
        AtomKind::Mutation => Some(Mutation(h.id)),
        _ => None,
    }
}

/// Extract a [`Mutation`] or raise a TypeError.
pub fn require_mutation(args: &[JsValue], idx: usize) -> JsResult<Mutation> {
    extract_mutation(args.get_or_undefined(idx)).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("expected a mutation atom handle"),
        )
    })
}

/// Extract the raw [`AtomHandle`] (id + kind) from a JS value.
pub fn extract_handle(value: &JsValue) -> Option<AtomHandle> {
    let obj = value.as_object()?;
    let h = obj.downcast_ref::<AtomHandle>()?;
    Some(AtomHandle { id: h.id, kind: h.kind })
}

/// Build the per-store `{ get, set }` JS context object that closures receive
/// as their first argument. Takes the reactive core directly — `get`/`set`
/// only ever call `read` / `set_source` / `invoke_mutation` (core-only), never
/// the subscriber graph, so the independent `SubscriberGraph` need not be
/// threaded in here.
pub fn build_store_context_object(
    context: &mut Context,
    core: Rc<RefCell<ReactiveCore>>,
) -> JsResult<JsObject> {
    let proto = context.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());

    let core_for_get = core.clone();
    let get_fn = unsafe {
        boa_engine::native_function::NativeFunction::from_closure(move |_this, args, ctx| {
            let readable = require_readable::<JsValue>(args, 0)?;
            Ok(core_for_get.borrow().read(readable, ctx))
        })
    };
    let get_obj = boa_engine::object::FunctionObjectBuilder::new(context.realm(), get_fn)
        .name(js_string!("get"))
        .length(1)
        .build();
    let get_desc = PropertyDescriptor::builder()
        .value(get_obj)
        .writable(true)
        .enumerable(false)
        .configurable(true)
        .build();
    obj.insert_property(js_string!("get"), get_desc);

    let core_for_set = core.clone();
    let set_fn = unsafe {
        boa_engine::native_function::NativeFunction::from_closure(move |_this, args, ctx| {
            let handle = extract_handle(args.get_or_undefined(0)).ok_or_else(|| {
                JsError::from(
                    JsNativeError::typ().with_message("expected an atom handle"),
                )
            })?;
            match handle.kind {
                AtomKind::Mutation => {
                    let mutation = Mutation(handle.id);
                    let ctx_obj = build_store_context_object(ctx, core_for_set.clone())?;
                    let mut invoke_args: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
                    invoke_args.push(ctx_obj.into());
                    if let Some(extra) = args.get(1..) {
                        invoke_args.extend_from_slice(extra);
                    }
                    core_for_set.borrow().invoke_mutation(mutation, &invoke_args, ctx)
                }
                AtomKind::Source => {
                    let value = args.get_or_undefined(1).clone();
                    core_for_set
                        .borrow()
                        .set_source(Source::<JsValue>::from_id(handle.id), value);
                    Ok(JsValue::undefined())
                }
                AtomKind::Derived => Err(JsError::from(
                    JsNativeError::typ().with_message("cannot set a derived atom"),
                )),
            }
        })
    };
    let set_obj = boa_engine::object::FunctionObjectBuilder::new(context.realm(), set_fn)
        .name(js_string!("set"))
        .length(2)
        .build();
    let set_desc = PropertyDescriptor::builder()
        .value(set_obj)
        .writable(true)
        .enumerable(false)
        .configurable(true)
        .build();
    obj.insert_property(js_string!("set"), set_desc);

    Ok(obj)
}

/// Convenience alias used by some view helpers — `Attribute::all()` etc.
#[allow(dead_code)]
fn _use_attribute() {}
