use std::cell::RefCell;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{js_string, Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::js_value::{FromJs, IntoJs};

mod store;

pub use store::Store;
pub use store::{ReactiveCore, ReactiveReadStore, ReactiveReadJsContext, SubscriberIndexStore};

/// Unique identifier for a reactive atom. Private to the reactive module —
/// all biz code addresses atoms via the typed handles (`Source<T>`,
/// `Derived<T>`, `Mutation`, `Readable<T>`) or the erased `AnyReadable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AtomId(u32);

// ---------------------------------------------------------------------------
// JsValue marshaling.
//
// Reactive handles cross the JS<->Rust boundary as boa opaque objects whose
// `JsData` payload *is* the handle itself (`Source<JsValue>` /
// `Derived<JsValue>` / `Mutation`).  The concrete type distinguishes the atom
// kind, so no separate kind tag is needed.  Wrap/unwrap goes through the
// unified [`crate::core::js_value::FromJs`] / [`crate::core::js_value::IntoJs`]
// traits; the private opaque wrappers are never named outside this module.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Typed atom handles — the Rust type system encodes the atom kind.
//
// `T` is a phantom type parameter: it exists only for compile-time type
// safety.  The Store stores `JsValue` for all atoms; `T` is erased to
// `PhantomData<fn() -> T>` (covariant, no Send/Sync/'static overhead).
//
// The inner `AtomId` and the `.id()` / `from_id` accessors are module-private:
// external code addresses handles opaquely (passing them to store methods) or
// converts to `AnyReadable` for the dependency-tracking layer.
// ---------------------------------------------------------------------------

/// Handle for a source atom (writable, never stale).
#[derive(Debug)]
pub struct Source<T>(AtomId, PhantomData<fn() -> T>);

/// Handle for a derived atom (lazy, recomputes on read when stale).
#[derive(Debug)]
pub struct Derived<T>(AtomId, PhantomData<fn() -> T>);

impl<T> Source<T> {
    #[inline]
    fn id(&self) -> AtomId {
        self.0
    }

    fn from_id(id: AtomId) -> Self {
        Source(id, PhantomData)
    }
}

impl<T> Derived<T> {
    #[inline]
    fn id(&self) -> AtomId {
        self.0
    }

    fn from_id(id: AtomId) -> Self {
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

/// Handle for a mutation atom (callable side-effect closure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mutation(AtomId);

/// Read-only reference to either a [`Source<T>`] or a [`Derived<T>`].  Used by
/// `Val<T>::Reactive`, `Store::read`, and the subscriber graph.
pub enum Readable<T> {
    Source(Source<T>),
    Derived(Derived<T>),
}

impl<T> Readable<T> {
    #[inline]
    fn id(&self) -> AtomId {
        match self {
            Readable::Source(s) => s.0,
            Readable::Derived(d) => d.0,
        }
    }

    /// Erase the phantom type parameter, yielding an [`AnyReadable`].
    /// Used by the dependency-tracking layer (which works with erased
    /// identities, since a subscriber may depend on atoms of mixed `T`).
    #[inline]
    pub fn to_any(&self) -> AnyReadable {
        match self {
            Readable::Source(s) => Readable::Source(Source::from_id(s.0)),
            Readable::Derived(d) => Readable::Derived(Derived::from_id(d.0)),
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
/// that is not decoded via [`FromJs`](crate::core::js_value::FromJs).
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
// JsValue marshaling impls.
//
// Handles cross the JS<->Rust boundary as boa opaques.  Because the handles
// themselves must stay `Copy` (they are passed around extensively in Rust) but
// `JsData` types cannot be `Copy`, the opaque payload is a tiny private
// non-`Copy` wrapper (`JsSource` / `JsDerived` / `JsMutation`) carrying just
// the `AtomId`.  The wrapper type distinguishes the atom kind, so no separate
// kind tag is needed.  All wrap/unwrap goes through the `IntoJs` /
// `FromJs` traits below — the wrappers are never named outside this
// module.
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
struct JsSource(AtomId);

#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
struct JsDerived(AtomId);

#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
struct JsMutation(AtomId);

fn wrap_opaque<T: boa_engine::object::NativeObject>(
    data: T,
    ctx: &mut Context,
) -> JsValue {
    let proto = ctx.intrinsics().constructors().object().prototype();
    JsObject::from_proto_and_data(proto, data).into()
}

impl<T> IntoJs for Source<T> {
    fn into_js(self, ctx: &mut Context) -> JsValue {
        wrap_opaque(JsSource(self.id()), ctx)
    }
}

impl<T> IntoJs for Derived<T> {
    fn into_js(self, ctx: &mut Context) -> JsValue {
        wrap_opaque(JsDerived(self.id()), ctx)
    }
}

impl IntoJs for Mutation {
    fn into_js(self, ctx: &mut Context) -> JsValue {
        wrap_opaque(JsMutation(self.0), ctx)
    }
}

impl FromJs for Source<JsValue> {
    fn from_js(value: &JsValue) -> Result<Self, JsError> {
        let obj = value.as_object().ok_or_else(|| {
            crate::core::js_value::type_error("a source atom handle")
        })?;
        let id = obj
            .downcast_ref::<JsSource>()
            .map(|s| s.0)
            .ok_or_else(|| crate::core::js_value::type_error("a source atom handle"))?;
        Ok(Source::from_id(id))
    }
}

impl FromJs for Derived<JsValue> {
    fn from_js(value: &JsValue) -> Result<Self, JsError> {
        let obj = value.as_object().ok_or_else(|| {
            crate::core::js_value::type_error("a derived atom handle")
        })?;
        let id = obj
            .downcast_ref::<JsDerived>()
            .map(|d| d.0)
            .ok_or_else(|| crate::core::js_value::type_error("a derived atom handle"))?;
        Ok(Derived::from_id(id))
    }
}

impl FromJs for Mutation {
    fn from_js(value: &JsValue) -> Result<Self, JsError> {
        let obj = value.as_object().ok_or_else(|| {
            crate::core::js_value::type_error("a mutation atom handle")
        })?;
        let id = obj
            .downcast_ref::<JsMutation>()
            .map(|m| m.0)
            .ok_or_else(|| crate::core::js_value::type_error("a mutation atom handle"))?;
        Ok(Mutation(id))
    }
}

impl<T> FromJs for Readable<T> {
    fn from_js(value: &JsValue) -> Result<Self, JsError> {
        let obj = value.as_object().ok_or_else(|| {
            crate::core::js_value::type_error("a source or derived atom handle")
        })?;
        if let Some(s) = obj.downcast_ref::<JsSource>() {
            return Ok(Readable::Source(Source::from_id(s.0)));
        }
        if let Some(d) = obj.downcast_ref::<JsDerived>() {
            return Ok(Readable::Derived(Derived::from_id(d.0)));
        }
        Err(crate::core::js_value::type_error(
            "a source or derived atom handle",
        ))
    }
}

/// Recover the private `AtomId` of an `AnyReadable`. Module-private; used by
/// the store capability faces to bridge erased handles to the internal
/// id-keyed maps.
fn atom_id_of(readable: AnyReadable) -> AtomId {
    readable.id()
}

/// Build an `AnyReadable` from a private id (used by the flush engine, which
/// produces stale ids internally and must surface them as erased handles).
fn any_readable_of(id: AtomId) -> AnyReadable {
    // Stale atoms from the flush engine are sources or deriveds — neither
    // kind nor value is recoverable from the id alone, and the dirty-
    // subscriber lookup only needs identity, so encode as a Source variant.
    Readable::Source(Source::from_id(id))
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
            let readable = AnyReadable::from_js(args.get_or_undefined(0))?;
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
            let v = args.get_or_undefined(0);
            if let Ok(mutation) = Mutation::from_js(v) {
                let ctx_obj = build_store_context_object(ctx, core_for_set.clone())?;
                let mut invoke_args: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
                invoke_args.push(ctx_obj.into());
                if let Some(extra) = args.get(1..) {
                    invoke_args.extend_from_slice(extra);
                }
                return core_for_set
                    .borrow()
                    .invoke_mutation(mutation, &invoke_args, ctx);
            }
            if let Ok(readable) = AnyReadable::from_js(v) {
                return match readable {
                    AnyReadable::Source(source) => {
                        let value = args.get_or_undefined(1).clone();
                        core_for_set.borrow().set_source(source, value);
                        Ok(JsValue::undefined())
                    }
                    AnyReadable::Derived(_) => Err(JsError::from(
                        JsNativeError::typ().with_message("cannot set a derived atom"),
                    )),
                };
            }
            Err(JsError::from(
                JsNativeError::typ().with_message("expected an atom handle"),
            ))
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
