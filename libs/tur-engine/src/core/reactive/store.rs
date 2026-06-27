use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsResult, JsValue};

use super::{AtomId, Derived, Mutation, Readable, Source, SubscriberId};

// ---------------------------------------------------------------------------
// Data sub-structs.  Each is a by-value group inside `ReactiveCore`; fields
// keep interior mutability so `&self` methods can mutate.  These group the
// reactive state into cohesive, named concerns.  The methods on `ReactiveCore`
// span them because reads / writes / flush are genuinely interwoven — the
// grouping is organizational, not an isolation boundary (the one truly
// separable concern, the subscriber index, lives in its own `SubscriberGraph`
// below).
// ---------------------------------------------------------------------------

/// Atom identity + value/closure storage.
struct AtomRegistry {
    values: RefCell<HashMap<AtomId, JsValue>>,
    closures: RefCell<HashMap<AtomId, JsFunction>>,
    next_id: Cell<u32>,
}

impl AtomRegistry {
    fn new() -> Self {
        AtomRegistry {
            values: RefCell::new(HashMap::new()),
            closures: RefCell::new(HashMap::new()),
            next_id: Cell::new(1),
        }
    }

    fn alloc_id(&self) -> AtomId {
        let id = AtomId(self.next_id.get());
        self.next_id.set(id.0 + 1);
        id
    }
}

/// Derived-recomputation graph: per-derived dependency sets, the reverse
/// `dependents` edges, the stale-derived set, and the reentrancy tracker
/// stack used while a derived closure runs (to discover its deps).
struct DerivedGraph {
    derived_deps: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    dependents: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    stale_deriveds: RefCell<HashSet<AtomId>>,
    tracker_stack: RefCell<Vec<HashSet<AtomId>>>,
}

impl DerivedGraph {
    fn new() -> Self {
        DerivedGraph {
            derived_deps: RefCell::new(HashMap::new()),
            dependents: RefCell::new(HashMap::new()),
            stale_deriveds: RefCell::new(HashSet::new()),
            tracker_stack: RefCell::new(Vec::new()),
        }
    }
}

/// Flush/dirty state: stale source atoms, the "a source changed" flag, and
/// the shared host-dirty signal (mirrors the element tree's dirty flag).
struct FlushState {
    stale_sources: RefCell<HashSet<AtomId>>,
    source_changed: Cell<bool>,
    host_dirty: Rc<Cell<bool>>,
}

impl FlushState {
    fn new(host_dirty: Rc<Cell<bool>>) -> Self {
        FlushState {
            stale_sources: RefCell::new(HashSet::new()),
            source_changed: Cell::new(false),
            host_dirty,
        }
    }
}

// ---------------------------------------------------------------------------
// SubscriberGraph — the ONE cleanly-separable concern.  Owns the
// atom↔subscriber edge index in its own `Rc<RefCell<..>>`, fully independent
// of the reactive core.  `SubscriberIndex` wraps this directly, so the write
// entry point (`set_subscriber_deps`) and the dirty query (`dirty_subscribers`)
// operate on genuinely separate data — not a view over the core blob.
// ---------------------------------------------------------------------------

pub(crate) struct SubscriberGraph {
    atom_to_subs: RefCell<HashMap<AtomId, HashSet<SubscriberId>>>,
    sub_to_atoms: RefCell<HashMap<SubscriberId, HashSet<AtomId>>>,
}

impl SubscriberGraph {
    fn new() -> Self {
        SubscriberGraph {
            atom_to_subs: RefCell::new(HashMap::new()),
            sub_to_atoms: RefCell::new(HashMap::new()),
        }
    }

    fn subscribe_edge(&self, atom: AtomId, sub: SubscriberId) {
        self.atom_to_subs
            .borrow_mut()
            .entry(atom)
            .or_default()
            .insert(sub);
        self.sub_to_atoms
            .borrow_mut()
            .entry(sub)
            .or_default()
            .insert(atom);
    }

    /// Atomically replace a subscriber's dependency set. Removes edges that are
    /// no longer present and adds new ones, diffing against the previous set.
    /// This is the single explicit write entry point for the atom↔subscriber
    /// index (replacing the old `clear_subscriber` + ambient auto-subscribe).
    pub fn set_subscriber_deps(&self, sub: SubscriberId, deps: HashSet<AtomId>) {
        let old = self
            .sub_to_atoms
            .borrow_mut()
            .insert(sub, deps.clone())
            .unwrap_or_default();

        // Remove edges for atoms no longer depended on.
        for atom in &old {
            if !deps.contains(atom) {
                let should_remove = {
                    let mut map = self.atom_to_subs.borrow_mut();
                    if let Some(subs) = map.get_mut(atom) {
                        subs.remove(&sub);
                        subs.is_empty()
                    } else {
                        false
                    }
                };
                if should_remove {
                    self.atom_to_subs.borrow_mut().remove(atom);
                }
            }
        }

        // Add edges for newly-declared deps.
        for atom in &deps {
            if !old.contains(atom) {
                self.subscribe_edge(*atom, sub);
            }
        }
    }

    pub fn dirty_subscribers(&self, atoms: &HashSet<AtomId>) -> HashSet<SubscriberId> {
        let mut out = HashSet::new();
        for atom in atoms {
            if let Some(subs) = self.atom_to_subs.borrow().get(atom) {
                out.extend(subs);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// ReactiveCore — the cohesive reactive machinery (atom registry + derived
// graph + flush state).  Methods here span the sub-structs because a read may
// recompute a derived (touching the graph + values), a write propagates
// invalidation (values + flush + graph), and flush drains across sources and
// deriveds.  This is the slimmed-down successor to `StoreInternal` (the
// subscriber fields moved out into `SubscriberGraph`).
//
// Methods are `pub(crate)` because the `{get,set}` JS-context object built by
// [`super::build_store_context_object`] captures a clone of the core and calls
// them from native closures during derived recompute.
// ---------------------------------------------------------------------------

pub struct ReactiveCore {
    atoms: AtomRegistry,
    graph: DerivedGraph,
    flush: FlushState,
    weak_self: RefCell<Weak<RefCell<ReactiveCore>>>,
}

impl ReactiveCore {
    fn new(host_dirty: Rc<Cell<bool>>) -> Self {
        ReactiveCore {
            atoms: AtomRegistry::new(),
            graph: DerivedGraph::new(),
            flush: FlushState::new(host_dirty),
            weak_self: RefCell::new(Weak::new()),
        }
    }

    pub(crate) fn source<T>(&self, value: JsValue) -> Source<T> {
        let id = self.atoms.alloc_id();
        self.atoms.values.borrow_mut().insert(id, value);
        Source(id, PhantomData)
    }

    pub(crate) fn derive<T>(&self, closure: JsFunction) -> Derived<T> {
        let id = self.atoms.alloc_id();
        self.atoms.closures.borrow_mut().insert(id, closure);
        self.graph.stale_deriveds.borrow_mut().insert(id);
        Derived(id, PhantomData)
    }

    pub(crate) fn mutate(&self, closure: JsFunction) -> Mutation {
        let id = self.atoms.alloc_id();
        self.atoms.closures.borrow_mut().insert(id, closure);
        Mutation(id)
    }

    pub(crate) fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        let id = readable.id();

        if let Readable::Derived(_) = readable {
            self.ensure_computed(id, ctx);
        }

        if let Some(top) = self.graph.tracker_stack.borrow_mut().last_mut() {
            top.insert(id);
        }

        self.atoms
            .values
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or(JsValue::undefined())
    }

    fn ensure_computed(&self, id: AtomId, ctx: &mut Context) {
        if !self.graph.stale_deriveds.borrow().contains(&id) {
            return;
        }

        let Some(closure) = self.atoms.closures.borrow().get(&id).cloned() else {
            return;
        };

        self.graph.tracker_stack.borrow_mut().push(HashSet::new());

        let store_ctx_obj = self.build_store_ctx_obj(ctx);

        let result = closure
            .call(
                &JsValue::undefined(),
                std::slice::from_ref(&store_ctx_obj),
                ctx,
            )
            .unwrap_or_else(|_| JsValue::undefined());

        let new_deps = self.graph.tracker_stack.borrow_mut().pop().unwrap();

        if let Some(old_deps) = self.graph.derived_deps.borrow().get(&id).cloned() {
            for dep in &old_deps {
                if let Some(set) = self.graph.dependents.borrow_mut().get_mut(dep) {
                    set.remove(&id);
                }
            }
        }
        self.graph
            .derived_deps
            .borrow_mut()
            .insert(id, new_deps.clone());
        for dep in &new_deps {
            self.graph
                .dependents
                .borrow_mut()
                .entry(*dep)
                .or_default()
                .insert(id);
        }

        self.atoms.values.borrow_mut().insert(id, result);
        self.graph.stale_deriveds.borrow_mut().remove(&id);
    }

    fn build_store_ctx_obj(&self, ctx: &mut Context) -> JsValue {
        let core = self
            .weak_self
            .borrow()
            .upgrade()
            .expect("store must be alive during recompute");
        super::build_store_context_object(ctx, core)
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined())
    }

    pub(crate) fn get_cached<T>(&self, readable: Readable<T>) -> JsValue {
        self.atoms
            .values
            .borrow()
            .get(&readable.id())
            .cloned()
            .unwrap_or(JsValue::undefined())
    }

    pub(crate) fn set_source<T>(&self, source: Source<T>, value: JsValue) {
        let id = source.0;
        let prev = self.atoms.values.borrow().get(&id).cloned();
        if prev.as_ref() == Some(&value) {
            return;
        }
        self.atoms.values.borrow_mut().insert(id, value);
        self.flush.source_changed.set(true);

        let mut queue = vec![id];
        let mut visited = HashSet::new();
        while let Some(atom) = queue.pop() {
            if !visited.insert(atom) {
                continue;
            }
            if atom == id {
                self.flush.stale_sources.borrow_mut().insert(atom);
            } else {
                self.graph.stale_deriveds.borrow_mut().insert(atom);
            }
            if let Some(deps) = self.graph.dependents.borrow().get(&atom).cloned() {
                for dep in deps {
                    queue.push(dep);
                }
            }
        }

        self.flush.host_dirty.set(true);
    }

    pub(crate) fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        let closure = self.atoms.closures.borrow().get(&mutation.0).cloned();
        let Some(closure) = closure else {
            return Ok(JsValue::undefined());
        };
        closure.call(&JsValue::undefined(), args, ctx)
    }

    pub(crate) fn flush(&self) -> HashSet<AtomId> {
        if !self.flush.source_changed.get() {
            return HashSet::new();
        }
        self.flush.source_changed.set(false);

        let mut stale = self.flush.stale_sources.borrow().clone();
        stale.extend(self.graph.stale_deriveds.borrow().iter().copied());

        self.flush.stale_sources.borrow_mut().clear();

        stale
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.flush.source_changed.get()
    }
}

impl std::fmt::Debug for ReactiveCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactiveCore")
            .field("atom_count", &self.atoms.values.borrow().len())
            .field(
                "stale_count",
                &(self.flush.stale_sources.borrow().len()
                    + self.graph.stale_deriveds.borrow().len()),
            )
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Store — the composite owner `{ core, graph }`.  Held by `TurJsContext` /
// `ElementTree` and used by the layout driver for orchestration (invoking
// pending mutations, building the JS-context object) and for handing out
// capability faces.  Atom creation is NOT on `Store` — it lives on the
// [`ReactiveBridgeStore`] face (the JS bridge is the sole atom minter).
// Cheap to clone (two `Rc` bumps).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Store {
    core: Rc<RefCell<ReactiveCore>>,
    graph: Rc<RefCell<SubscriberGraph>>,
}

impl Store {
    pub fn new(host_dirty: Rc<Cell<bool>>) -> Store {
        let core = Rc::new(RefCell::new(ReactiveCore::new(host_dirty)));
        let graph = Rc::new(RefCell::new(SubscriberGraph::new()));
        *core.borrow().weak_self.borrow_mut() = Rc::downgrade(&core);
        Store { core, graph }
    }

    pub fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        self.core.borrow().invoke_mutation(mutation, args, ctx)
    }

    // ----- capability faces ---------------------------------------------------
    //
    // Narrow views, each handing out only the `Rc` to the piece it wraps.
    // Produced here (the trusted owner) so business code — which never
    // receives a `Store` — cannot reach these knobs.

    /// Read-only view for business code: resolve atom values without the
    /// ability to create atoms, write, or touch the subscriber index / engine.
    pub(crate) fn read_only(&self) -> ReactiveRead {
        ReactiveRead {
            core: self.core.clone(),
        }
    }

    /// View over the atom↔subscriber index (the independent `SubscriberGraph`):
    /// declare a subscriber's deps and query which subscribers depend on a set
    /// of atoms. Held by `SubscribeCx` (write) and the layout driver (read).
    pub(crate) fn subscriber_index(&self) -> SubscriberIndex {
        SubscriberIndex {
            graph: self.graph.clone(),
        }
    }

    /// View over the stale/dirty engine: drain pending source changes and
    /// report whether any are pending. Held by the layout driver.
    pub(crate) fn flush_engine(&self) -> FlushEngine {
        FlushEngine {
            core: self.core.clone(),
        }
    }

    /// Build the per-store `{get,set}` JS-context object that mutation/derived
    /// closures receive. Wraps [`super::build_store_context_object`] with the
    /// core handle so callers never need the raw `Rc<RefCell<ReactiveCore>>`
    /// (which would otherwise expose `flush`/`has_pending`).
    pub(crate) fn ctx_object(&self, ctx: &mut Context) -> JsResult<JsObject> {
        super::build_store_context_object(ctx, self.core.clone())
    }

    /// The JS-bridge capability face: atom creation (`source`/`derive`/
    /// `mutate`), read, write (`set_source`/`invoke_mutation`), and ctx-object
    /// building. This is the **only** way to mint atoms — `Store` itself no
    /// longer exposes creation, mirroring how `SubscriberIndex` is the only
    /// way to reach `set_subscriber_deps`.
    pub(crate) fn bridge(&self) -> ReactiveBridgeStore {
        ReactiveBridgeStore {
            core: self.core.clone(),
        }
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.core.borrow().fmt(f)
    }
}

// ---------------------------------------------------------------------------
// ReactiveRead — read-only capability face for business code (element impls,
// layout, widgets, handlers). Wraps the core but exposes only value reads.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReactiveRead {
    core: Rc<RefCell<ReactiveCore>>,
}

impl ReactiveRead {
    pub fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        self.core.borrow().read(readable, ctx)
    }

    pub fn get_cached<T>(&self, readable: Readable<T>) -> JsValue {
        self.core.borrow().get_cached(readable)
    }
}

// ---------------------------------------------------------------------------
// SubscriberIndex — capability face over the **independent** subscriber graph.
// `set_subscriber_deps` (used by `SubscribeCx`) and `dirty_subscribers` (used
// by the layout driver) operate on `SubscriberGraph`'s own data, fully
// decoupled from the reactive core.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SubscriberIndex {
    graph: Rc<RefCell<SubscriberGraph>>,
}

impl SubscriberIndex {
    pub fn set_subscriber_deps(&self, sub: SubscriberId, deps: HashSet<AtomId>) {
        self.graph.borrow().set_subscriber_deps(sub, deps);
    }

    pub fn dirty_subscribers(&self, atoms: &HashSet<AtomId>) -> HashSet<SubscriberId> {
        self.graph.borrow().dirty_subscribers(atoms)
    }
}

// ---------------------------------------------------------------------------
// FlushEngine — capability face over the stale/dirty engine. Drains pending
// source changes (`flush`) and reports whether any are pending (`has_pending`).
// Held by the layout driver.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FlushEngine {
    core: Rc<RefCell<ReactiveCore>>,
}

impl FlushEngine {
    pub fn flush(&self) -> HashSet<AtomId> {
        self.core.borrow().flush()
    }

    pub fn has_pending(&self) -> bool {
        self.core.borrow().has_pending()
    }
}

// ---------------------------------------------------------------------------
// ReactiveBridgeStore — the JS-bridge capability face: the sole entry point
// for atom creation (`source`/`derive`/`mutate`), plus the read/write surface
// the bridge needs (`read`/`set_source`/`invoke_mutation`) and ctx-object
// building. Produced exclusively via [`Store::bridge`]; `Store` itself exposes
// none of these, so atom minting is gated behind the bridge.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReactiveBridgeStore {
    core: Rc<RefCell<ReactiveCore>>,
}

impl ReactiveBridgeStore {
    pub fn source<T>(&self, value: JsValue) -> Source<T> {
        self.core.borrow().source(value)
    }

    pub fn derive<T>(&self, closure: JsFunction) -> Derived<T> {
        self.core.borrow().derive(closure)
    }

    pub fn mutate(&self, closure: JsFunction) -> Mutation {
        self.core.borrow().mutate(closure)
    }

    pub fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        self.core.borrow().read(readable, ctx)
    }

    pub fn set_source<T>(&self, source: Source<T>, value: JsValue) {
        self.core.borrow().set_source(source, value);
    }

    pub fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        self.core.borrow().invoke_mutation(mutation, args, ctx)
    }

    /// Build the per-store `{get,set}` JS-context object (wraps
    /// [`super::build_store_context_object`]). The bridge uses this when
    /// invoking a mutation.
    pub fn ctx_object(&self, ctx: &mut Context) -> JsResult<JsObject> {
        super::build_store_context_object(ctx, self.core.clone())
    }
}
