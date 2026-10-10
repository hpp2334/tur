use std::hash::Hash;
use std::marker::PhantomData;

use crate::core::edgy::value::Value;

mod store;

pub use store::Store;
pub use store::{
    FlushEngineStore, ReactiveBridgeStore, ReactiveReadStore, ScalarRead, SharedReactive, StoreKv,
    SubscriberIndexStore, WatchDispatchStore,
};

/// Unique identifier for a reactive atom — the single id space for ALL atoms
/// of an instance, allocated from one shared counter (so every map keyed by
/// bare `AtomId` is collision-free across stores). Private to the reactive
/// module — all biz code addresses atoms via the typed handles
/// (`Source<T>`, `Derived<T>`, `Mutation`, `Readable<T>`) or the erased
/// `AnyReadable`.
///
/// An atom is its seed (initial value for a source, closure for a derived /
/// mutation), kept in the shared registry ([`SharedReactive`]). Its value
/// lives in a store's KV and materializes on first touch: each store holds
/// its own copy, so the same id in two stores is two independent values.
/// Engine "environment" atoms (`viewportSize$` …) are ordinary atoms whose
/// backing the engine writes through the tree's current store (see
/// [`SharedReactive`] docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AtomId(pub(crate) u32);



// ---------------------------------------------------------------------------
// JsValue marshaling.
//
// Reactive handles cross the JS<->Rust boundary as boa opaque objects whose
// `JsData` payload *is* the handle itself (`Source<Value>` /
// `Derived<Value>` / `Mutation`).  The concrete type distinguishes the atom
// kind, so no separate kind tag is needed.  Wrap/unwrap goes through the
// unified [`crate::core::js_runtime::js_value::FromJs`] / [`crate::core::js_runtime::js_value::IntoJs`]
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
// safety.  The Store stores `Value` for all atoms; `T` is erased to
// `PhantomData<fn() -> T>` (covariant, no Send/Sync/'static overhead).
//
// A handle is a bare id addressing a seed in the shared registry; the
// value materializes into whichever store the read/write flows through.
// Callers never distinguish — the store resolves on read/write.
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
    pub(crate) fn id(&self) -> AtomId {
        self.0
    }

    pub(crate) fn from_id(id: AtomId) -> Self {
        Source(id, PhantomData)
    }
}

impl<T> Derived<T> {
    #[inline]
    pub(crate) fn id(&self) -> AtomId {
        self.0
    }

    pub(crate) fn from_id(id: AtomId) -> Self {
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

impl Mutation {
    pub(crate) fn id(&self) -> AtomId {
        self.0
    }

    /// Rebuild a mutation handle from its raw id — the rut rail's crossing
    /// (the ids ARE the atoms; see [`crate::core::rut_runtime::mutation_of`]).
    pub(crate) fn from_id(id: AtomId) -> Self {
        Mutation(id)
    }
}

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

/// Untyped readable — carries a raw native [`Value`] (e.g. a list or map
/// atom) that is not decoded via [`FromValue`](crate::core::edgy::FromValue).
pub type AnyReadable = Readable<Value>;

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
