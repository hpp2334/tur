use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use super::any_readable_of;
use super::atom_id_of;
use super::{AnyReadable, AtomId, Derived, Mutation, Readable, Source, SubscriberId};
use crate::core::edgy::watch::WatcherRegistry;

// ---------------------------------------------------------------------------
// Data sub-structs.  Each is a by-value group; fields keep interior
// mutability so `&self` methods can mutate.  These group the reactive state
// into cohesive, named concerns.  The methods on [`SharedReactive`] span them
// because reads / writes / flush are genuinely interwoven — the grouping is
// organizational, not an isolation boundary (the one truly separable concern,
// the subscriber index, lives in its own `SubscriberGraph`).
//
// MULTI-STORE LAYOUT: the machinery below is split in two —
//   * [`SharedReactive`] — instance-wide: the atom-id counter, the id→seed
//     registry, the derived graph, the flush state, the subscriber graph, the
//     owner map (single-home atoms) and the holder index (staleness across
//     stores). Because atom ids are allocated from one shared counter, every
//     map keyed by bare `AtomId` (derived deps, dependents, stale sets,
//     subscriber edges) is collision-free across stores, and a write in one
//     store invalidates deriveds materialized in any other (the dependents
//     walk is shared).
//   * [`StoreKv`] — per-store: the atom VALUE map. This is what makes a store
//     a store: the same id materialized in two stores yields two independent
//     values.
// ---------------------------------------------------------------------------

/// The seed of an atom — the data that exists before any store materializes
/// it. Sources carry their initial value; deriveds / mutations carry their
/// closure. Seeds live centrally (shared across stores) so the same id can
/// materialize independently in any store.
#[derive(Clone)]
enum Seed {
    Source(JsValue),
    Derived(Closure),
    Mutate(Closure),
}

/// Per-atom closure payload — backs both `derive` and `mutate` atoms.
///
/// `Js` is the JS-engine path: a boa `JsFunction` invoked with the per-store
/// `{get, set}` JsObject as its first argument (the legacy / JS-bridge
/// surface that every `tur:core` `derive(fn)` / `mutate(fn)` call mints).
///
/// `DeriveRust` / `MutateRust` are the Rust-native paths exposed to plugins
/// via [`ReactiveBridgeStore::build_derive`] / [`ReactiveBridgeStore::build_mutate`]:
/// they receive a typed capability face (`&ReactiveReadStore` for derive,
/// `&ReactiveBridgeStore` for mutate) directly, skipping the JsObject
/// round-trip entirely. Reads still flow through `SharedReactive::read_by_id`,
/// so the auto-dependency tracker works for free.
///
/// The kind is encoded in the variant — a `DeriveRust` closure is never
/// dispatched via `invoke_mutation` (and vice versa). The handle types
/// (`Derived<T>` vs `Mutation`) make cross-kind dispatch unreachable via the
/// public API; the panics in `ensure_computed` / `invoke_mutation` are
/// defensive guards against an engine-internal invariant violation.
#[derive(Clone)]
enum Closure {
    Js(JsFunction),
    DeriveRust(Rc<DeriveRustFn>),
    MutateRust(Rc<MutateRustFn>),
}

/// Rust-native derive closure signature: receives a read-only face (gets
/// only) and the JS engine. Returns the recomputed value.
type DeriveRustFn = dyn Fn(&ReactiveReadStore, &mut Context) -> JsResult<JsValue>;

/// Rust-native mutate closure signature: receives the read+write bridge
/// face, the user-supplied args (no `{get,set}` JsObject prepended), and
/// the JS engine. Returns whatever the closure chooses to hand back to JS.
type MutateRustFn = dyn Fn(&ReactiveBridgeStore, &[JsValue], &mut Context) -> JsResult<JsValue>;

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
    app_dirty: Rc<Cell<bool>>,
}

impl FlushState {
    fn new(app_dirty: Rc<Cell<bool>>) -> Self {
        FlushState {
            stale_sources: RefCell::new(HashSet::new()),
            source_changed: Cell::new(false),
            app_dirty,
        }
    }
}

// ---------------------------------------------------------------------------
// SubscriberGraph — the ONE cleanly-separable concern.  Owns the
// atom↔subscriber edge index in its own data, fully independent of the
// reactive core.  `SubscriberIndexStore` wraps this directly, so the write
// entry point (`set_subscriber_deps`) and the dirty query (`dirty_subscribers`)
// operate on genuinely separate data — not a view over the core blob.
// ---------------------------------------------------------------------------

pub struct SubscriberGraph {
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
    /// This is the single explicit write entry point for the atom<->subscriber
    /// index (replacing the old `clear_subscriber` + ambient auto-subscribe).
    fn set_subscriber_deps(&self, sub: SubscriberId, deps: HashSet<AtomId>) {
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

    fn dirty_subscribers(&self, atoms: &HashSet<AtomId>) -> HashSet<SubscriberId> {
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
// StoreKv — the per-store state container. A store IS its KV: the map of
// materialized atom VALUES, keyed by the shared atom-id space. Seeds live in
// [`SharedReactive`]; a value lands here when a store reads (materializing
// the seed) or writes the atom.
// ---------------------------------------------------------------------------

pub struct StoreKv {
    values: RefCell<HashMap<AtomId, JsValue>>,
}

impl StoreKv {
    fn new() -> Self {
        StoreKv {
            values: RefCell::new(HashMap::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// SharedReactive — the instance-wide reactive machinery shared by every
// store of one JS realm: atom id allocation, the id→seed registry, the
// derived graph, the flush state, the subscriber graph, owner routing
// (single-home atoms) and the holder index (cross-store staleness).
//
// Routing rule (the ONE rule of the model):
//   * An id with an **owner** entry (engine/plugin-minted atoms, e.g.
//     `viewportSize$`) has a single value home: the owning store's KV. Reads
//     and writes from any store route there.
//   * An id **without** an owner (a JS `source()` / `derive()` / `mutate()`
//     declaration) materializes into whichever store first reads/writes it —
//     each store's KV holds its own independent value for the id.
//
// All fields have interior mutability, so the struct is shared as a plain
// `Rc<SharedReactive>` and every method takes `&self` — nested recompute
// (a derived reading another derived) is safe without re-entrant borrows.
// ---------------------------------------------------------------------------

pub struct SharedReactive {
    next_atom: Cell<u32>,
    seeds: RefCell<HashMap<AtomId, Seed>>,
    /// Single-home atoms: id → the store whose KV holds the value. Populated
    /// only by the Rust mint path (`ReactiveBridgeStore::source` etc.) —
    /// engine/plugin atoms are owned by the engine store.
    owners: RefCell<HashMap<AtomId, Rc<StoreKv>>>,
    /// Stores that have materialized each id (for staleness: invalidating a
    /// derived must drop every store's cached copy). Weak so a dropped store
    /// doesn't leak.
    holders: RefCell<HashMap<AtomId, Vec<Weak<StoreKv>>>>,
    graph: DerivedGraph,
    flush: FlushState,
    subscribers: SubscriberGraph,
    /// `watch()` registry — non-element subscribers (see `edgy::watch`).
    watchers: WatcherRegistry,
    weak_self: RefCell<Weak<SharedReactive>>,
}

impl SharedReactive {
    fn new(app_dirty: Rc<Cell<bool>>) -> Self {
        SharedReactive {
            next_atom: Cell::new(1),
            seeds: RefCell::new(HashMap::new()),
            owners: RefCell::new(HashMap::new()),
            holders: RefCell::new(HashMap::new()),
            graph: DerivedGraph::new(),
            flush: FlushState::new(app_dirty),
            subscribers: SubscriberGraph::new(),
            watchers: WatcherRegistry::new(),
            weak_self: RefCell::new(Weak::new()),
        }
    }

    fn self_rc(&self) -> Rc<SharedReactive> {
        self.weak_self
            .borrow()
            .upgrade()
            .expect("shared reactive machinery must be alive during recompute")
    }

    fn alloc_id(&self) -> AtomId {
        let id = AtomId(self.next_atom.get());
        self.next_atom.set(id.0 + 1);
        id
    }

    /// The KV a given read/write should flow through: the owner's for
    /// single-home atoms, otherwise the caller's `via` KV (materializing
    /// declarations live wherever they're touched).
    fn route(&self, id: AtomId, via: &Rc<StoreKv>) -> Rc<StoreKv> {
        self.owners
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_else(|| via.clone())
    }

    fn register_holder(&self, id: AtomId, kv: &Rc<StoreKv>) {
        let mut holders = self.holders.borrow_mut();
        let list = holders.entry(id).or_default();
        if !list
            .iter()
            .any(|w| w.upgrade().is_some_and(|k| Rc::ptr_eq(&k, kv)))
        {
            list.push(Rc::downgrade(kv));
        }
    }

    // ----- minting ----------------------------------------------------------

    /// Mint a **declaration** (the JS `source(v)` / `derive(fn)` / `mutate(fn)`
    /// path): id + central seed, no owner — the atom materializes into
    /// whichever store first touches it.
    fn decl(&self, seed: Seed) -> AtomId {
        let id = self.alloc_id();
        if let Seed::Derived(_) = seed {
            self.graph.stale_deriveds.borrow_mut().insert(id);
        }
        self.seeds.borrow_mut().insert(id, seed);
        id
    }

    /// Mint an **owned** atom (the Rust bridge path): id + seed + value home
    /// in `kv`. Reads from any store route here.
    fn owned(&self, kv: &Rc<StoreKv>, seed: Seed, initial: Option<JsValue>) -> AtomId {
        let id = self.alloc_id();
        if let Seed::Derived(_) = seed {
            self.graph.stale_deriveds.borrow_mut().insert(id);
        }
        self.seeds.borrow_mut().insert(id, seed);
        self.owners.borrow_mut().insert(id, kv.clone());
        if let Some(v) = initial {
            kv.values.borrow_mut().insert(id, v);
        }
        id
    }

    // ----- read / write -----------------------------------------------------

    pub(crate) fn read_by_id(&self, id: AtomId, via: &Rc<StoreKv>, ctx: &mut Context) -> JsValue {
        // Auto-dependency tracking: any read inside a running derived closure
        // records the dep.
        if let Some(top) = self.graph.tracker_stack.borrow_mut().last_mut() {
            top.insert(id);
        }

        let kv = self.route(id, via);
        if kv.values.borrow().contains_key(&id) && !self.graph.stale_deriveds.borrow().contains(&id)
        {
            return kv
                .values
                .borrow()
                .get(&id)
                .cloned()
                .unwrap_or(JsValue::undefined());
        }

        // Not materialized here (or stale): materialize from the seed.
        let seed = self.seeds.borrow().get(&id).cloned();
        match seed {
            Some(Seed::Source(initial)) => {
                kv.values.borrow_mut().insert(id, initial.clone());
                self.register_holder(id, &kv);
                initial
            }
            Some(Seed::Derived(_)) => {
                self.ensure_computed(id, ctx, &kv);
                kv.values
                    .borrow()
                    .get(&id)
                    .cloned()
                    .unwrap_or(JsValue::undefined())
            }
            _ => JsValue::undefined(),
        }
    }

    fn ensure_computed(&self, id: AtomId, ctx: &mut Context, kv: &Rc<StoreKv>) {
        if !self.graph.stale_deriveds.borrow().contains(&id) {
            return;
        }

        let closure = match self.seeds.borrow().get(&id) {
            Some(Seed::Derived(c)) => c.clone(),
            _ => return,
        };

        self.graph.tracker_stack.borrow_mut().push(HashSet::new());

        // Dispatch on the closure kind. The Js branch builds the per-store
        // `{get, set}` JsObject (declarations materialize into this derived's
        // routed store) and calls the JS closure with it; the DeriveRust
        // branch hands the closure a typed `&ReactiveReadStore` face (no
        // JsObject round-trip). The MutateRust branch is unreachable here —
        // derive handles never carry a MutateRust closure.
        let result = match closure {
            Closure::Js(f) => {
                let store_ctx_obj = self.build_store_ctx_obj(ctx, kv.clone());
                f.call(
                    &JsValue::undefined(),
                    std::slice::from_ref(&store_ctx_obj),
                    ctx,
                )
                .unwrap_or_else(|_| JsValue::undefined())
            }
            Closure::DeriveRust(f) => {
                let read_store = ReactiveReadStore {
                    shared: self.self_rc(),
                    default: kv.clone(),
                };
                f(&read_store, ctx).unwrap_or_else(|_| JsValue::undefined())
            }
            Closure::MutateRust(_) => {
                panic!(
                    "ensure_computed called on a MutateRust closure (atom id {:?}) — \
                     derive handles must be paired with Js or DeriveRust closures",
                    id
                );
            }
        };

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

        kv.values.borrow_mut().insert(id, result);
        self.register_holder(id, kv);
        self.graph.stale_deriveds.borrow_mut().remove(&id);
    }

    fn build_store_ctx_obj(&self, ctx: &mut Context, kv: Rc<StoreKv>) -> JsValue {
        super::build_store_context_object(ctx, self.self_rc(), kv)
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined())
    }

    pub(crate) fn write_by_id(
        &self,
        id: AtomId,
        via: &Rc<StoreKv>,
        value: JsValue,
    ) -> JsResult<()> {
        let kv = self.route(id, via);
        let prev = kv.values.borrow().get(&id).cloned();
        if prev.as_ref() == Some(&value) {
            return Ok(());
        }
        // Watch-loop guard: while a watcher callback is delivering, a write
        // that re-invalidates a delivering watcher's watched atom throws at
        // the JS call site (the write is rejected, no state changes).
        if let Some(message) = self.detect_watch_loop(id) {
            return Err(JsError::from(JsNativeError::typ().with_message(message)));
        }
        kv.values.borrow_mut().insert(id, value);
        self.register_holder(id, &kv);
        self.flush.source_changed.set(true);

        // Propagate invalidation: mark all dependents stale AND drop every
        // store's cached copy of them (per-KV caches must not survive a dep
        // change in another store).
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
                let holders = self
                    .holders
                    .borrow()
                    .get(&atom)
                    .cloned()
                    .unwrap_or_default();
                for w in holders {
                    if let Some(hkv) = w.upgrade() {
                        hkv.values.borrow_mut().remove(&atom);
                    }
                }
            }
            if let Some(deps) = self.graph.dependents.borrow().get(&atom).cloned() {
                for dep in deps {
                    queue.push(dep);
                }
            }
        }

        self.flush.app_dirty.set(true);
        Ok(())
    }

    /// Watch-loop detection: while any watcher callback is delivering, a
    /// write to `written` is rejected if it transitively invalidates a
    /// delivering watcher's watched atom. The dependents closure is walked
    /// read-only (no recompute, no state change) — a `derive` on the path
    /// is not recomputed, only its dependency edges are followed.
    fn detect_watch_loop(&self, written: AtomId) -> Option<String> {
        if !self.watchers.is_delivering() {
            return None;
        }
        let watched = self.watchers.delivering_watched();
        if watched.is_empty() {
            return None;
        }
        let mut seen: HashSet<AtomId> = HashSet::new();
        let mut queue = vec![written];
        while let Some(atom) = queue.pop() {
            if !seen.insert(atom) {
                continue;
            }
            if let Some(deps) = self.graph.dependents.borrow().get(&atom).cloned() {
                for dep in deps {
                    queue.push(dep);
                }
            }
        }
        for w in watched {
            if seen.contains(&w) {
                return Some(format!(
                    "watch loop detected: a watch callback wrote atom {} which \
                     re-invalidates the atom it watches — watchers must not write \
                     what they watch (write to a different atom, or restructure the \
                     flow so the watched atom only changes from outside the callback)",
                    w.0
                ));
            }
        }
        None
    }

    // ----- mutation invoke ----------------------------------------------------

    /// Invoke a mutation atom. `args` are the **user-supplied** args only
    /// (no leading `{get,set}` JsObject) — the JsObject is constructed
    /// internally and prepended **only** for `Js`-variant closures. The
    /// `MutateRust` variant receives the user args verbatim alongside a
    /// typed `&ReactiveBridgeStore` face. `via` is the KV declarations
    /// materialize into (the invoking store).
    pub(crate) fn invoke_mutation_by_id(
        &self,
        id: AtomId,
        via: &Rc<StoreKv>,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        let closure = match self.seeds.borrow().get(&id) {
            Some(Seed::Mutate(c)) => c.clone(),
            _ => return Ok(JsValue::undefined()),
        };
        // Arm the watch-loop guard when this mutation is a watcher callback,
        // so writes inside the closure run `detect_watch_loop`.
        let armed = self.watchers.arm(id);
        let outcome = match closure {
            Closure::Js(f) => {
                // Build the per-store `{get, set}` JsObject and prepend it
                // before invoking — JS closures expect `(ctx, ...args)`.
                let ctx_obj = self.build_store_ctx_obj(ctx, via.clone());
                let mut full: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
                full.push(ctx_obj);
                full.extend_from_slice(args);
                f.call(&JsValue::undefined(), &full, ctx)
            }
            Closure::MutateRust(f) => {
                // Skip the JsObject entirely; hand the closure the bridge
                // face plus the user args verbatim.
                let bridge = ReactiveBridgeStore {
                    store: Store {
                        shared: self.self_rc(),
                        kv: via.clone(),
                    },
                };
                f(&bridge, args, ctx)
            }
            Closure::DeriveRust(_) => {
                panic!(
                    "invoke_mutation called on a DeriveRust closure (atom id {:?}) — \
                     mutation handles must be paired with Js or MutateRust closures",
                    id
                );
            }
        };
        if armed {
            self.watchers.disarm();
        }
        outcome
    }

    // ----- flush ---------------------------------------------------------------

    fn flush(&self) -> HashSet<AtomId> {
        if !self.flush.source_changed.get() {
            return HashSet::new();
        }
        self.flush.source_changed.set(false);

        let mut stale = self.flush.stale_sources.borrow().clone();
        stale.extend(self.graph.stale_deriveds.borrow().iter().copied());

        self.flush.stale_sources.borrow_mut().clear();

        stale
    }

    fn has_pending(&self) -> bool {
        self.flush.source_changed.get()
    }
}

impl std::fmt::Debug for SharedReactive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedReactive")
            .field("seed_count", &self.seeds.borrow().len())
            .field(
                "stale_count",
                &(self.flush.stale_sources.borrow().len()
                    + self.graph.stale_deriveds.borrow().len()),
            )
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Store — the composite `{ shared, kv }`. One instance per JS realm is
// created by the engine (the **engine store**, which plugin- and
// engine-minted atoms call home); `createStore()` from JS mints further
// stores over the same shared machinery (`Store::spawn`). Held by
// `TurInstanceContext` / `ElementTree` and used by the layout driver for
// orchestration (invoking pending mutations, building the JS-context object)
// and for handing out capability faces.  Cheap to clone (two `Rc` bumps).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Store {
    pub(crate) shared: Rc<SharedReactive>,
    pub(crate) kv: Rc<StoreKv>,
}

impl Store {
    pub fn new(app_dirty: Rc<Cell<bool>>) -> Store {
        let shared = Rc::new(SharedReactive::new(app_dirty));
        *shared.weak_self.borrow_mut() = Rc::downgrade(&shared);
        Store {
            shared,
            kv: Rc::new(StoreKv::new()),
        }
    }

    /// Mint a new store over this store's shared machinery — `createStore()`.
    /// The new store shares the id space / seed registry / derived graph /
    /// flush state / subscriber graph, but owns a fresh KV: an atom
    /// materialized here is independent of the same atom in another store.
    pub fn spawn(&self) -> Store {
        Store {
            shared: self.shared.clone(),
            kv: Rc::new(StoreKv::new()),
        }
    }

    /// Whether both stores belong to the same instance (share machinery).
    pub fn same_instance(&self, other: &Store) -> bool {
        Rc::ptr_eq(&self.shared, &other.shared)
    }

    pub(crate) fn shared(&self) -> Rc<SharedReactive> {
        self.shared.clone()
    }

    pub(crate) fn kv_handle(&self) -> Rc<StoreKv> {
        self.kv.clone()
    }

    /// Invoke a mutation atom. `args` are the **user-supplied** args only
    /// (no leading `{get,set}` JsObject) — see
    /// [`SharedReactive::invoke_mutation_by_id`] for the dispatch details.
    /// Declarations materialize into THIS store's KV during the invocation.
    pub fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        self.shared
            .invoke_mutation_by_id(mutation.id(), &self.kv, args, ctx)
    }

    // ----- capability faces ---------------------------------------------------
    //
    // Narrow views, each handing out only the `Rc` to the piece it wraps.
    // Produced here (the trusted owner) so business code — which never
    // receives a `Store` — cannot reach these knobs.

    /// Read-only view for business code: resolve atom values without the
    /// ability to create atoms, write, or touch the subscriber index / engine.
    /// Declarations materialize into this face's default store (the mounted
    /// store for tree-driven flows).
    pub fn read_only(&self) -> ReactiveReadStore {
        ReactiveReadStore {
            shared: self.shared.clone(),
            default: self.kv.clone(),
        }
    }

    /// View over the atom↔subscriber index (the shared `SubscriberGraph`):
    /// declare a subscriber's deps and query which subscribers depend on a set
    /// of atoms. Held by `SubscribeCx` (write) and the layout driver (read).
    pub fn subscriber_index(&self) -> SubscriberIndexStore {
        SubscriberIndexStore {
            shared: self.shared.clone(),
        }
    }

    /// View over the stale/dirty engine: drain pending source changes and
    /// report whether any are pending. Held by the layout driver.
    pub fn flush_engine(&self) -> FlushEngineStore {
        FlushEngineStore {
            shared: self.shared.clone(),
        }
    }

    /// View over the `watch()` registry (delivery side): which watcher
    /// callbacks are due for a set of dirtied atoms. Held by the flush loop.
    pub fn watch_dispatch(&self) -> WatchDispatchStore {
        WatchDispatchStore {
            shared: self.shared.clone(),
        }
    }

    /// Build the per-store `{get,set}` JS-context object that mutation/derived
    /// closures receive. Wraps [`super::build_store_context_object`] with the
    /// shared machinery + this store's KV (declarations materialize here).
    pub fn ctx_object(&self, ctx: &mut Context) -> JsResult<JsObject> {
        super::build_store_context_object(ctx, self.shared.clone(), self.kv.clone())
    }

    /// The JS-bridge capability face: atom creation (`source`/`derive`/
    /// `mutate`), read, write (`set_source`/`invoke_mutation`), and ctx-object
    /// building. This is the **only** way to mint atoms — `Store` itself no
    /// longer exposes creation, mirroring how `SubscriberIndexStore` is the only
    /// way to reach `set_subscriber_deps`. Rust-minted atoms register eagerly
    /// in the face's store (engine plugins use the engine store).
    pub fn bridge(&self) -> ReactiveBridgeStore {
        ReactiveBridgeStore {
            store: self.clone(),
        }
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.shared.fmt(f)
    }
}

// ---------------------------------------------------------------------------
// ReactiveReadStore — read-only capability face for business code (element impls,
// layout, views, handlers). Wraps the shared machinery + a default KV:
// owned atoms route to their owner; declarations materialize into `default`
// (for tree-driven flows, the mounted store).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReactiveReadStore {
    shared: Rc<SharedReactive>,
    default: Rc<StoreKv>,
}

impl ReactiveReadStore {
    pub fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        let (id, derived) = match readable {
            Readable::Source(s) => (s.id(), false),
            Readable::Derived(d) => (d.id(), true),
        };
        let _ = derived; // read_by_id handles staleness uniformly
        self.shared.read_by_id(id, &self.default, ctx)
    }
}

impl std::fmt::Debug for ReactiveReadStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactiveReadStore").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// ReactiveReadJsContext — the layout-only engine face. Holds a `ReactiveReadStore`
// (the read-only reactive face) plus a borrow of the JS engine, and exposes
// **only** `read`. Layout code — which holds `&mut ReactiveReadJsContext` via
// `LayoutContext` — therefore cannot reach `set` / `invoke_mutation` / global
// registration; the raw `Context` lives only at the trusted app/bridge boundary.
// ---------------------------------------------------------------------------

pub struct ReactiveReadJsContext<'a> {
    read: ReactiveReadStore,
    boa: &'a mut Context,
}

impl<'a> ReactiveReadJsContext<'a> {
    pub fn new(read: ReactiveReadStore, boa: &'a mut Context) -> Self {
        ReactiveReadJsContext { read, boa }
    }

    /// Resolve a `Readable<T>` to its current JS value, lazily recomputing a
    /// stale `Derived` if necessary. This is the only operation exposed to the
    /// layout phase — there is no `set`, no mutation, no engine mutation here.
    pub fn read<T>(&mut self, readable: Readable<T>) -> JsValue {
        self.read.read(readable, self.boa)
    }

    /// Borrow the underlying JS `Context`. Layout-phase build (e.g. LazyList
    // remount) needs it to call the JS item builder and construct
    /// `ElementObject`s. The read-only guarantee is weakened only for code
    /// that chooses to call mutating JS.
    #[allow(dead_code)]
    pub fn boa_mut(&mut self) -> &mut Context {
        self.boa
    }
}

// ---------------------------------------------------------------------------
// SubscriberIndexStore — capability face over the **shared** subscriber graph.
// `set_subscriber_deps` (used by `SubscribeCx`) and `dirty_subscribers` (used
// by the layout driver) operate on `SubscriberGraph`'s own data, fully
// decoupled from the reactive core.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SubscriberIndexStore {
    shared: Rc<SharedReactive>,
}

impl SubscriberIndexStore {
    pub fn set_subscriber_deps(&self, sub: SubscriberId, deps: HashSet<AnyReadable>) {
        let deps: HashSet<AtomId> = deps.into_iter().map(atom_id_of).collect();
        self.shared.subscribers.set_subscriber_deps(sub, deps);
    }

    pub fn dirty_subscribers(&self, atoms: &HashSet<AnyReadable>) -> HashSet<SubscriberId> {
        let ids: HashSet<AtomId> = atoms.iter().copied().map(atom_id_of).collect();
        self.shared.subscribers.dirty_subscribers(&ids)
    }
}

// ---------------------------------------------------------------------------
// FlushEngineStore — capability face over the stale/dirty engine. Drains pending
// source changes (`flush`) and reports whether any are pending (`has_pending`).
// Held by the layout driver.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FlushEngineStore {
    shared: Rc<SharedReactive>,
}

impl FlushEngineStore {
    pub fn flush_atoms(&self) -> HashSet<AnyReadable> {
        self.shared
            .flush()
            .into_iter()
            .map(any_readable_of)
            .collect()
    }

    pub fn has_pending(&self) -> bool {
        self.shared.has_pending()
    }
}

// ---------------------------------------------------------------------------
// WatchDispatchStore — capability face over the `watch()` registry's delivery
// side. `due_callbacks` returns the callbacks owed for a set of dirtied atoms,
// coalesced to at most once per flush epoch per watcher. The flush loop pushes
// them onto the mutation queue — same invocation rail as every other mutation.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WatchDispatchStore {
    shared: Rc<SharedReactive>,
}

impl WatchDispatchStore {
    /// Watcher callbacks due for `dirties` at flush epoch `epoch`. Stamps
    /// each returned watcher's epoch (a watcher delivers at most once per
    /// epoch — the convergence backstop for indirect write cycles).
    pub fn due_callbacks(&self, dirties: &HashSet<AnyReadable>, epoch: u64) -> Vec<Mutation> {
        let ids: HashSet<AtomId> = dirties.iter().copied().map(atom_id_of).collect();
        self.shared.watchers.take_due(&ids, epoch)
    }
}

// ---------------------------------------------------------------------------
// ReactiveBridgeStore — the JS-bridge capability face: the sole entry point
// for atom creation (`source`/`derive`/`mutate`), plus the read/write surface
// the bridge needs (`read`/`set_source`/`invoke_mutation`) and ctx-object
// building. Produced exclusively via [`Store::bridge`]; `Store` itself exposes
// none of these, so atom minting is gated behind the bridge.
//
// Rust-minted atoms (the `source`/`derive`/`mutate`/`build_*` methods) are
// **owned** by this face's store; JS-declaration minting (`decl_source` /
// `decl_derive` / `decl_mutate`, used by the `tur:core` bridge fns) produces
// owner-less ids that materialize per store.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReactiveBridgeStore {
    store: Store,
}

impl ReactiveBridgeStore {
    /// Mint an owned source atom (value lives in this store's KV; reads from
    /// any store route here).
    pub fn source<T>(&self, value: JsValue) -> Source<T> {
        let id = self
            .store
            .shared
            .owned(&self.store.kv, Seed::Source(value.clone()), Some(value));
        Source(id, PhantomData)
    }

    /// Mint an owned derived atom.
    pub fn derive<T>(&self, closure: JsFunction) -> Derived<T> {
        let id = self
            .store
            .shared
            .owned(&self.store.kv, Seed::Derived(Closure::Js(closure)), None);
        Derived(id, PhantomData)
    }

    /// Mint an owned mutation atom.
    pub fn mutate(&self, closure: JsFunction) -> Mutation {
        Mutation(
            self.store
                .shared
                .owned(&self.store.kv, Seed::Mutate(Closure::Js(closure)), None),
        )
    }

    /// Mint a **declaration** — the JS `source(v)` path. No owner: the atom
    /// materializes into whichever store first reads/writes it.
    pub fn decl_source<T>(&self, value: JsValue) -> Source<T> {
        Source(self.store.shared.decl(Seed::Source(value)), PhantomData)
    }

    /// Mint a **declaration** — the JS `derive(fn)` path.
    pub fn decl_derive<T>(&self, closure: JsFunction) -> Derived<T> {
        Derived(
            self.store.shared.decl(Seed::Derived(Closure::Js(closure))),
            PhantomData,
        )
    }

    /// Mint a **declaration** — the JS `mutate(fn)` path.
    pub fn decl_mutate(&self, closure: JsFunction) -> Mutation {
        Mutation(self.store.shared.decl(Seed::Mutate(Closure::Js(closure))))
    }

    /// Mint a derived atom whose value is computed by a **Rust closure**.
    /// The closure receives a read-only reactive face (`&ReactiveReadStore`)
    /// at recompute time, with no `{get, set}` JsObject round-trip. Reads
    /// inside the closure still flow through the same machinery the JS
    /// `derive(fn)` path uses, so automatic dependency tracking works
    /// identically (and nested `ensure_computed` for deriveds read by this
    /// closure is safe — all reactive methods are `&self`).
    ///
    /// Plugins reach this via
    /// [`PluginContext::reactive`](crate::core::plugin::PluginContext::reactive)
    /// and typically expose the returned handle to JS via
    /// [`PluginContext::register_global`] or as a bridge-fn return value;
    /// JS then reads it through `store.get(handle)`.
    pub fn build_derive<F>(&self, closure: F) -> Derived<JsValue>
    where
        F: Fn(&ReactiveReadStore, &mut Context) -> JsResult<JsValue> + 'static,
    {
        let id = self.store.shared.owned(
            &self.store.kv,
            Seed::Derived(Closure::DeriveRust(Rc::new(closure))),
            None,
        );
        Derived(id, PhantomData)
    }

    /// Mint a mutation atom whose logic is a **Rust closure**. The closure
    /// receives this same `&ReactiveBridgeStore` face (so it can read and
    /// write atoms directly) plus the user-supplied args at invocation time,
    /// with no `{get, set}` JsObject round-trip.
    ///
    /// Plugins reach this via
    /// [`PluginContext::reactive`](crate::core::plugin::PluginContext::reactive);
    /// JS invokes the mutation through `store.set(mutation, ...args)`,
    /// which routes the user args here verbatim.
    pub fn build_mutate<F>(&self, closure: F) -> Mutation
    where
        F: Fn(&ReactiveBridgeStore, &[JsValue], &mut Context) -> JsResult<JsValue> + 'static,
    {
        Mutation(self.store.shared.owned(
            &self.store.kv,
            Seed::Mutate(Closure::MutateRust(Rc::new(closure))),
            None,
        ))
    }

    /// `watch(readable, cb)` support: register a watcher over `watched` with
    /// `callback` (already minted via `decl_mutate` by the bridge fn), and
    /// mint the `start$` / `stop$` control mutations. Returns
    /// `(start_mutation, stop_mutation)`.
    ///
    /// The control closures capture a `Weak` to the shared machinery — a
    /// strong capture would create an Rc cycle (the closure lives in the
    /// shared seed registry, which the closure would hold alive).
    ///
    /// `start$` additionally materializes the watched atom once (through the
    /// invoking store): a declared-but-never-computed derived otherwise sits
    /// in the stale set, and the next *unrelated* source write would count it
    /// as dirtied and fire the watcher spuriously. After the materializing
    /// read the atom is clean, so only real dep changes re-dirty it.
    pub(crate) fn register_watch(
        &self,
        watched: AnyReadable,
        callback: Mutation,
    ) -> (Mutation, Mutation) {
        let watcher = self
            .store
            .shared
            .watchers
            .register(atom_id_of(watched), callback);

        let weak_start = Rc::downgrade(&self.store.shared);
        let start = self.build_mutate(move |bridge, _args, ctx| {
            let Some(shared) = weak_start.upgrade() else {
                return Ok(JsValue::undefined());
            };
            if let Some((watched_id, _)) = shared.watchers.activate(watcher) {
                let _ = shared.read_by_id(watched_id, &bridge.store.kv, ctx);
            }
            Ok(JsValue::undefined())
        });

        let weak_stop = Rc::downgrade(&self.store.shared);
        let stop = self.build_mutate(move |_bridge, _args, _ctx| {
            if let Some(shared) = weak_stop.upgrade() {
                shared.watchers.deactivate(watcher);
            }
            Ok(JsValue::undefined())
        });

        (start, stop)
    }

    pub fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        self.store.read_only().read(readable, ctx)
    }

    pub fn set_source<T>(&self, source: Source<T>, value: JsValue) -> JsResult<()> {
        self.store
            .shared
            .write_by_id(source.id(), &self.store.kv, value)
    }

    /// Invoke a mutation atom. `args` are the **user-supplied** args only
    /// — for `Js`-variant mutations the per-store `{get, set}` JsObject is
    /// constructed and prepended internally (so callers must NOT prepend it
    /// themselves); for `MutateRust`-variant mutations the args are passed
    /// verbatim alongside the bridge face.
    pub fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        self.store.invoke_mutation(mutation, args, ctx)
    }

    /// Build the per-store `{get,set}` JS-context object (wraps
    /// [`super::build_store_context_object`]). Exposed for the rare host
    /// bridge that needs to mint a ctx object directly; ordinary mutation
    /// invocation constructs it internally.
    pub fn ctx_object(&self, ctx: &mut Context) -> JsResult<JsObject> {
        self.store.ctx_object(ctx)
    }
}
