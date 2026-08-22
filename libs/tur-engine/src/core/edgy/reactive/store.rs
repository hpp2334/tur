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
// LAYOUT: the machinery is split in two —
//   * [`SharedReactive`] — instance-wide: the atom-id counter, the id→seed
//     registry, the derived graph (incl. the invalidation generations that
//     keep per-store caches coherent), the flush state and the subscriber
//     graph. Because atom ids are allocated from one shared counter, every
//     map keyed by bare `AtomId` (derived deps, dependents, stale sets,
//     subscriber edges) is collision-free, and a write invalidates every
//     materialized copy (the dependents walk is shared). The machinery
//     holds NO store references and NO values — every method takes the
//     caller's store (`via`) per call. One instance has exactly ONE store
//     (created by the engine at build, handed to `start({ store })`); the
//     substrate keeps the store-per-KV shape so the value home is always an
//     ordinary store KV, never an instance-scoped side slot.
//   * [`StoreKv`] — per-store: the atom VALUE map. This is what makes a
//     store a store — values live in a store KV, nowhere else.
//
// An atom is its seed: id + initial value or closure, no value anywhere
// until the store materializes it — and the store's KV holds the copy.
// Engine "environment" atoms (`viewportSize$` …) are ordinary atoms with a
// designated home: the backing source materializes in the INSTANCE store
// (the engine writes it via the ordinary `set_source` rail), and the public
// handle is a derive whose closure reads the backing through a captured
// engine read face — so the value resolves identically from every read
// path, with cache coherence provided by the generation rail (see
// `ReactiveBridgeStore::read_only`). No read path ever receives hidden
// engine values.
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
/// `dependents` edges, the stale-derived set, the reentrancy tracker
/// stack used while a derived closure runs (to discover its deps), and the
/// per-atom **generation** counter driving cross-store cache coherence.
struct DerivedGraph {
    derived_deps: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    dependents: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    /// Flush reporting: deriveds marked dirty since their last compute.
    /// NOT a cache-serving input — a store's cached slot is served iff its
    /// recorded generation matches the atom's current generation below.
    stale_deriveds: RefCell<HashSet<AtomId>>,
    tracker_stack: RefCell<Vec<HashSet<AtomId>>>,
    /// Derived atoms currently being recomputed. A read of an in-flight id
    /// means the closure (directly or through another derived) re-entered its
    /// own computation — pure-Rust closures have no JS frames, so without
    /// this guard that recurses to an OS thread-stack overflow.
    in_flight_derives: RefCell<Vec<AtomId>>,
    /// Cross-store cache coherence: the invalidation generation of each
    /// derived. Every invalidation (a write to a transitive dep) bumps the
    /// atom's generation; a `StoreKv` slot records the generation it was
    /// computed at, and a mismatch (or a missing slot) forces a recompute
    /// IN THAT STORE regardless of what any other store did. Sources are
    /// never bumped, so their slots are always fresh — one uniform rule.
    generations: RefCell<HashMap<AtomId, u64>>,
    next_generation: Cell<u64>,
}

impl DerivedGraph {
    fn new() -> Self {
        DerivedGraph {
            derived_deps: RefCell::new(HashMap::new()),
            dependents: RefCell::new(HashMap::new()),
            stale_deriveds: RefCell::new(HashSet::new()),
            tracker_stack: RefCell::new(Vec::new()),
            in_flight_derives: RefCell::new(Vec::new()),
            generations: RefCell::new(HashMap::new()),
            next_generation: Cell::new(1),
        }
    }

    /// The current invalidation generation of `atom` (0 = never
    /// invalidated).
    fn generation_of(&self, atom: AtomId) -> u64 {
        self.generations.borrow().get(&atom).copied().unwrap_or(0)
    }

    /// Mark `atom`'s cached copies (in every store) stale by bumping its
    /// generation.
    fn bump_generation(&self, atom: AtomId) {
        let epoch = self.next_generation.get();
        self.next_generation.set(epoch + 1);
        self.generations.borrow_mut().insert(atom, epoch);
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

    /// Drop a subscriber and every edge it declared. Called from the element
    /// tree's destroy paths — destroyed nodes never re-declare (the subscribe
    /// phase runs only during layout), so their last edge set would otherwise
    /// persist as phantom subscribers forever.
    fn remove_subscriber(&self, sub: SubscriberId) {
        let Some(atoms) = self.sub_to_atoms.borrow_mut().remove(&sub) else {
            return;
        };
        for atom in &atoms {
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

    /// Dev-tool introspection: `(live subscribers, total declared edges)`.
    fn stats(&self) -> (usize, usize) {
        let map = self.sub_to_atoms.borrow();
        let edges = map.values().map(|deps| deps.len()).sum();
        (map.len(), edges)
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
// materialized atom values, keyed by the shared atom-id space. Seeds live
// in [`SharedReactive`]; a value lands here when a store reads (materializing
// the seed) or writes the atom.
//
// Each slot records the **generation** it was computed at. Deriveds get
// their generation bumped by every invalidation (see [`DerivedGraph`]), so
// a slot is servable iff its generation matches the atom's current one —
// a missing or outdated slot recomputes in THIS store no matter what any
// other store did. Sources are never bumped: always fresh once present.
// ---------------------------------------------------------------------------

/// A materialized atom value + the invalidation generation it was computed
/// at (see [`StoreKv`] docs).
struct Slot {
    value: JsValue,
    epoch: u64,
}

pub struct StoreKv {
    values: RefCell<HashMap<AtomId, Slot>>,
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
// derived graph (dependency edges + invalidation generations), the flush
// state, and the subscriber graph.
//
// The machinery is store-free: every read/write/invoke takes the caller's
// store per call (`via`), and an id's value ALWAYS lives in a store's KV —
// an atom materializes into whichever store first reads/writes it (each
// store's KV holds its own independent value for the id).
//
// All fields have interior mutability, so the struct is shared as a plain
// `Rc<SharedReactive>` and every method takes `&self` — nested recompute
// (a derived reading another derived) is safe without re-entrant borrows.
// ---------------------------------------------------------------------------

pub struct SharedReactive {
    next_atom: Cell<u32>,
    seeds: RefCell<HashMap<AtomId, Seed>>,
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

    // ----- minting ----------------------------------------------------------

    /// Mint an atom: id + central seed, no value. The atom materializes
    /// into whichever store first touches it; each store's KV holds its own
    /// value. JS `source(v)` / `derive(fn)` / `mutate(fn)` and every Rust
    /// `bridge.build_*` mint through here alike.
    fn decl(&self, seed: Seed) -> AtomId {
        let id = self.alloc_id();
        if let Seed::Derived(_) = seed {
            self.graph.stale_deriveds.borrow_mut().insert(id);
        }
        self.seeds.borrow_mut().insert(id, seed);
        id
    }

    // ----- read / write -----------------------------------------------------

    pub(crate) fn read_by_id(
        &self,
        id: AtomId,
        via: &Rc<StoreKv>,
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        // Auto-dependency tracking: any read inside a running derived closure
        // records the dep.
        if let Some(top) = self.graph.tracker_stack.borrow_mut().last_mut() {
            top.insert(id);
        }

        // Serve the cached slot iff it is fresh: recorded at the atom's
        // current generation (sources are never bumped — always fresh once
        // present; deriveds go stale in THIS store the moment a transitive
        // dep is written anywhere).
        {
            let epoch = self.graph.generation_of(id);
            let values = via.values.borrow();
            if let Some(slot) = values.get(&id).filter(|s| s.epoch == epoch) {
                return Ok(slot.value.clone());
            }
        }

        // Not materialized here (or stale): materialize from the seed.
        let seed = self.seeds.borrow().get(&id).cloned();
        match seed {
            Some(Seed::Source(initial)) => {
                let epoch = self.graph.generation_of(id);
                via.values.borrow_mut().insert(
                    id,
                    Slot {
                        value: initial.clone(),
                        epoch,
                    },
                );
                Ok(initial)
            }
            Some(Seed::Derived(_)) => {
                self.ensure_computed(id, ctx, via)?;
                let epoch = self.graph.generation_of(id);
                let values = via.values.borrow();
                Ok(values
                    .get(&id)
                    .filter(|slot| slot.epoch == epoch)
                    .map(|slot| slot.value.clone())
                    .unwrap_or(JsValue::undefined()))
            }
            _ => Ok(JsValue::undefined()),
        }
    }

    fn ensure_computed(&self, id: AtomId, ctx: &mut Context, kv: &Rc<StoreKv>) -> JsResult<()> {
        // Fresh in THIS store already? (A compute elsewhere does not count —
        // per-store values.)
        let fresh_here = kv
            .values
            .borrow()
            .get(&id)
            .is_some_and(|s| s.epoch == self.graph.generation_of(id));
        if fresh_here {
            return Ok(());
        }
        // Cycle guard: re-entrant computation of this derive (its closure
        // reading itself directly or through another derived) would recurse
        // natively until the thread overflows — fail the read instead. The
        // error surfaces at the read site and the atom stays unmaterialized.
        if self.graph.in_flight_derives.borrow().contains(&id) {
            return Err(JsError::from(JsNativeError::typ().with_message(
                "cycle detected: derived atom re-entered its own computation",
            )));
        }

        let closure = match self.seeds.borrow().get(&id) {
            Some(Seed::Derived(c)) => c.clone(),
            _ => return Ok(()),
        };

        self.graph.tracker_stack.borrow_mut().push(HashSet::new());
        self.graph.in_flight_derives.borrow_mut().push(id);

        // Dispatch on the closure kind. The Js branch builds the per-store
        // `{get, set}` JsObject (declarations materialize into this derived's
        // store) and calls the JS closure with it; the DeriveRust branch
        // hands the closure a typed `&ReactiveReadStore` face (no JsObject
        // round-trip). The MutateRust branch is unreachable here — derive
        // handles never carry a MutateRust closure.
        let result = match closure {
            Closure::Js(f) => {
                let store_ctx_obj = self.build_store_ctx_obj(ctx, kv.clone());
                f.call(
                    &JsValue::undefined(),
                    std::slice::from_ref(&store_ctx_obj),
                    ctx,
                )
            }
            Closure::DeriveRust(f) => {
                let read_store = ReactiveReadStore {
                    shared: self.self_rc(),
                    default: kv.clone(),
                };
                f(&read_store, ctx)
            }
            Closure::MutateRust(_) => {
                panic!(
                    "ensure_computed called on a MutateRust closure (atom id {:?}) — \
                     derive handles must be paired with Js or DeriveRust closures",
                    id
                );
            }
        };

        // Unwind the reentrancy guards BEFORE propagating a possible error,
        // so a throwing closure cannot leak a tracker frame or an in-flight
        // marker into later computes.
        let new_deps = self.graph.tracker_stack.borrow_mut().pop().unwrap();
        self.graph.in_flight_derives.borrow_mut().pop();

        // Error propagation: a closure that throws fails the read at its
        // call site, and the atom stays stale + unmaterialized (no sticky
        // `undefined`) — the next read retries, so the derive recovers once
        // the underlying state is present.
        let result = result?;

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

        // Record the slot at the CURRENT generation, re-read after the
        // closure ran: a write to a dep that happened during the compute
        // already bumped the generation, so the slot lands already-stale and
        // the next read recomputes (no torn state).
        let epoch = self.graph.generation_of(id);
        kv.values.borrow_mut().insert(
            id,
            Slot {
                value: result,
                epoch,
            },
        );
        self.graph.stale_deriveds.borrow_mut().remove(&id);
        Ok(())
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
        let prev = via.values.borrow().get(&id).map(|s| s.value.clone());
        if prev.as_ref() == Some(&value) {
            return Ok(());
        }
        // Watch-loop guard: while a watcher callback is delivering, a write
        // that re-invalidates a delivering watcher's watched atom throws at
        // the JS call site (the write is rejected, no state changes).
        if let Some(message) = self.detect_watch_loop(id) {
            return Err(JsError::from(JsNativeError::typ().with_message(message)));
        }
        // A written source keeps its generation (sources are never bumped),
        // so the slot stays fresh wherever it is read.
        let epoch = self.graph.generation_of(id);
        via.values.borrow_mut().insert(id, Slot { value, epoch });
        self.flush.source_changed.set(true);

        // Propagate invalidation through the dependents closure: mark each
        // derived stale (flush reporting) and bump its generation, which
        // invalidates EVERY store's cached copy of it — a slot recorded at
        // an older generation fails the freshness check on its next read,
        // in whatever store holds it.
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
                self.graph.bump_generation(atom);
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
// Store — the composite `{ shared, kv }`. Each instance has exactly ONE
// store, created by the engine at build (the **instance store**, handed to
// the module's `start({ store })` and permanently bound to the
// instance-owned tree; engine- and plugin-minted atoms call home to it).
// Held by `TurInstanceContext` / `ElementTree` and used by the layout driver
// for orchestration (invoking pending mutations, building the JS-context
// object) and for handing out capability faces.  Cheap to clone (two `Rc`
// bumps).
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

    /// Drop a subscriber (element/fragment node) and every edge it declared.
    /// Called by the element tree's destroy paths.
    pub fn remove_subscriber(&self, sub: SubscriberId) {
        self.shared.subscribers.remove_subscriber(sub);
    }

    /// Dev-tool introspection: `(live subscribers, total declared edges)` on
    /// the shared graph.
    pub fn subscriber_stats(&self) -> (usize, usize) {
        self.shared.subscribers.stats()
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
// ReactiveReadStore — read-only capability face for business code (element
// impls, layout, views, handlers). Wraps the shared machinery + a default
// KV: atoms materialize into `default` (for tree-driven flows, the mounted
// store).
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
        // Rust-native face (layout reads + DeriveRust closures): a cycle
        // error can't propagate as JsResult here — fall back to undefined,
        // mirroring throwing-closure semantics.
        self.shared
            .read_by_id(id, &self.default, ctx)
            .unwrap_or(JsValue::undefined())
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
// for atom minting (`decl_source` / `decl_derive` / `decl_mutate` /
// `build_derive` / `build_mutate`), plus the read/write surface the bridge
// needs (`read`/`set_source`/`invoke_mutation`) and ctx-object building.
// Produced exclusively via [`Store::bridge`]; `Store` itself exposes none of
// these, so atom minting is gated behind the bridge. Every mint produces an
// id + seed in the shared registry — no value lands until a store touches
// the atom.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReactiveBridgeStore {
    store: Store,
}

impl ReactiveBridgeStore {
    /// Mint a source atom: id + seed carrying the initial value. No value
    /// lands anywhere — the atom materializes into whichever store first
    /// reads or writes it.
    pub fn decl_source<T>(&self, value: JsValue) -> Source<T> {
        Source(self.store.shared.decl(Seed::Source(value)), PhantomData)
    }

    /// Mint a derived atom: id + seed carrying the JS closure.
    pub fn decl_derive<T>(&self, closure: JsFunction) -> Derived<T> {
        Derived(
            self.store.shared.decl(Seed::Derived(Closure::Js(closure))),
            PhantomData,
        )
    }

    /// Mint a mutation atom: id + seed carrying the JS closure.
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
        let id = self
            .store
            .shared
            .decl(Seed::Derived(Closure::DeriveRust(Rc::new(closure))));
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
        Mutation(
            self.store
                .shared
                .decl(Seed::Mutate(Closure::MutateRust(Rc::new(closure)))),
        )
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
                // Materialization best-effort: a cyclic watched derived
                // errors here and simply stays unmaterialized this epoch.
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

    /// The bridge's own store (the **engine store** when the bridge came
    /// from `PluginContext::reactive` / `TurInstanceContext::reactive`),
    /// as a read-only face. The engine-atom pattern captures this in a
    /// `build_derive` handle closure so the handle — read from ANY store of
    /// the instance — resolves the backing's single engine-store value:
    /// ```text
    /// let backing = bridge.decl_source(initial);
    /// let engine_read = bridge.read_only();
    /// let handle = bridge.build_derive(move |_read, boa| {
    ///     Ok(engine_read.read(Readable::from(backing), boa))
    /// });
    /// // publish: bridge.set_source(backing, v) — the ordinary write rail
    /// ```
    /// Cross-store coherence rides the existing generation rail: the write
    /// bumps the handle's generation, so every store's cached copy
    /// recomputes on its next read.
    pub fn read_only(&self) -> ReactiveReadStore {
        self.store.read_only()
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
