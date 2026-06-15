use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsResult, JsValue};

use super::{AtomId, AtomKind};

/// Reactive store holding source values, derived closures, and the
/// dependency graph that drives fine-grained updates.
///
/// All state is wrapped in `RefCell` so that every accessor takes `&self`.
/// This allows reentrant calls — e.g. a derived closure that reads another
/// derived which must be recomputed first — without re-borrowing the outer
/// `Rc<RefCell<Store>>`.
pub struct Store {
    values: RefCell<HashMap<AtomId, JsValue>>,
    closures: RefCell<HashMap<AtomId, JsFunction>>,
    kinds: RefCell<HashMap<AtomId, AtomKind>>,
    derived_deps: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    dirty: RefCell<HashSet<AtomId>>,
    next_id: Cell<u32>,
    tracker_stack: RefCell<Vec<HashSet<AtomId>>>,
    /// Set to `true` whenever a source changes — used by the host
    /// (`TurAppInternal`) to know it should call `flush()` and dispatch
    /// updates.
    host_dirty: Rc<Cell<bool>>,
}

impl Store {
    pub fn new(host_dirty: Rc<Cell<bool>>) -> Self {
        Store {
            values: RefCell::new(HashMap::new()),
            closures: RefCell::new(HashMap::new()),
            kinds: RefCell::new(HashMap::new()),
            derived_deps: RefCell::new(HashMap::new()),
            dirty: RefCell::new(HashSet::new()),
            next_id: Cell::new(1),
            tracker_stack: RefCell::new(Vec::new()),
            host_dirty,
        }
    }

    fn alloc_id(&self) -> AtomId {
        let id = AtomId(self.next_id.get());
        self.next_id.set(id.0 + 1);
        id
    }

    pub fn source(&self, value: JsValue) -> AtomId {
        let id = self.alloc_id();
        self.values.borrow_mut().insert(id, value);
        self.kinds.borrow_mut().insert(id, AtomKind::Source);
        id
    }

    pub fn derive(&self, closure: JsFunction, ctx: &mut Context, store_ctx_obj: &JsValue) -> AtomId {
        let id = self.alloc_id();
        self.closures.borrow_mut().insert(id, closure.clone());
        self.kinds.borrow_mut().insert(id, AtomKind::Derived);

        // Pre-compute the initial value so subsequent reads (e.g. from
        // widget do_build) can return a cached JsValue without needing to
        // run the closure lazily.
        self.tracker_stack.borrow_mut().push(HashSet::new());
        let result = closure
            .call(&JsValue::undefined(), std::slice::from_ref(store_ctx_obj), ctx)
            .unwrap_or_else(|_| JsValue::undefined());
        let deps = self.tracker_stack.borrow_mut().pop().unwrap();
        self.values.borrow_mut().insert(id, result);
        self.derived_deps.borrow_mut().insert(id, deps);
        id
    }

    pub fn mutate(&self, closure: JsFunction) -> AtomId {
        let id = self.alloc_id();
        self.closures.borrow_mut().insert(id, closure);
        self.kinds.borrow_mut().insert(id, AtomKind::Mutation);
        id
    }

    pub fn kind_of(&self, id: AtomId) -> Option<AtomKind> {
        self.kinds.borrow().get(&id).copied()
    }

    /// Read the value of an atom. If a tracker is active (i.e. we are inside
    /// a derived recompute), record the dependency.
    pub fn get_tracked(&self, id: AtomId, _ctx: &mut Context) -> JsValue {
        if let Some(top) = self.tracker_stack.borrow_mut().last_mut() {
            top.insert(id);
        }
        self.values
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or(JsValue::undefined())
    }

    /// Read without tracking. Used by widget `do_build`/`do_update`.
    pub fn get_untracked(&self, id: AtomId, _ctx: &mut Context) -> JsValue {
        self.get_raw(id)
    }

    /// Read without tracking and without a boa `Context`.  Used by layout
    /// and paint to resolve reactive `Val<T>` values on the fly.
    pub fn get_raw(&self, id: AtomId) -> JsValue {
        self.values
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or(JsValue::undefined())
    }

    /// Write a new value to a source atom. Marks the atom dirty and signals
    /// the host.
    pub fn set_source(&self, id: AtomId, value: JsValue) {
        let prev = self.values.borrow().get(&id).cloned();
        if prev.as_ref() == Some(&value) {
            return;
        }
        self.values.borrow_mut().insert(id, value);
        self.dirty.borrow_mut().insert(id);
        self.host_dirty.set(true);
    }

    /// Invoke a mutation atom: call its closure with `args`. The closure is
    /// expected to take `(ctx, ...rest)` where `ctx` is the store's
    /// `{get, set}` JS object (passed as `args[0]`).
    pub fn invoke_mutation(
        &self,
        id: AtomId,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        let closure = self.closures.borrow().get(&id).cloned();
        let Some(closure) = closure else {
            return Ok(JsValue::undefined());
        };
        closure.call(&JsValue::undefined(), args, ctx)
    }

    /// Expand the dirty set through the derived dependency graph (BFS),
    /// recompute dirty deriveds (updating their dep sets), and return the
    /// full set of atoms whose consumers must be notified.
    pub fn flush(&self, ctx: &mut Context, store_ctx_obj: &JsValue) -> HashSet<AtomId> {
        if self.dirty.borrow().is_empty() {
            return HashSet::new();
        }

        // Step 1: BFS expansion through derived_deps.
        let initial: HashSet<AtomId> = self.dirty.borrow().iter().copied().collect();
        let mut expanded: HashSet<AtomId> = initial.clone();
        let mut queue: Vec<AtomId> = initial.iter().copied().collect();
        while let Some(key) = queue.pop() {
            for (d_id, deps) in self.derived_deps.borrow().iter() {
                if expanded.contains(d_id) {
                    continue;
                }
                if deps.contains(&key) {
                    expanded.insert(*d_id);
                    queue.push(*d_id);
                }
            }
        }

        // Step 2: recompute dirty deriveds (those reachable from a changed
        // source) with a tracker active, and update their dep sets.
        let dirty_deriveds: Vec<AtomId> = expanded
            .iter()
            .copied()
            .filter(|id| self.kinds.borrow().get(id) == Some(&AtomKind::Derived))
            .collect();

        let mut changed: HashSet<AtomId> = initial;
        for d_id in dirty_deriveds {
            let Some(closure) = self.closures.borrow().get(&d_id).cloned() else {
                continue;
            };
            self.tracker_stack.borrow_mut().push(HashSet::new());
            let result = closure
                .call(&JsValue::undefined(), std::slice::from_ref(store_ctx_obj), ctx)
                .unwrap_or_else(|_| JsValue::undefined());
            let new_deps = self.tracker_stack.borrow_mut().pop().unwrap();

            let prev = self.values.borrow().get(&d_id).cloned();
            let same = prev.as_ref() == Some(&result);
            self.values.borrow_mut().insert(d_id, result);
            self.derived_deps.borrow_mut().insert(d_id, new_deps);
            if !same {
                changed.insert(d_id);
            }
        }

        self.dirty.borrow_mut().clear();
        changed
    }

    pub fn has_pending(&self) -> bool {
        !self.dirty.borrow().is_empty()
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("atom_count", &self.values.borrow().len())
            .field("dirty_count", &self.dirty.borrow().len())
            .finish_non_exhaustive()
    }
}
