use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsResult, JsValue};

use super::{AtomId, Derived, Mutation, Readable, Source, SubscriberId};

// ---------------------------------------------------------------------------
// StoreInternal — the actual reactive state.  All methods take `&self` and
// use per-field interior mutability so reentrant calls (a derived closure
// reading another derived) work without re-borrowing the outer RefCell.
// ---------------------------------------------------------------------------

struct StoreInternal {
    values: RefCell<HashMap<AtomId, JsValue>>,
    closures: RefCell<HashMap<AtomId, JsFunction>>,
    derived_deps: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    dependents: RefCell<HashMap<AtomId, HashSet<AtomId>>>,
    stale_sources: RefCell<HashSet<AtomId>>,
    stale_deriveds: RefCell<HashSet<AtomId>>,
    next_id: Cell<u32>,
    tracker_stack: RefCell<Vec<HashSet<AtomId>>>,
    current_subscriber: Cell<Option<SubscriberId>>,
    atom_to_subs: RefCell<HashMap<AtomId, HashSet<SubscriberId>>>,
    sub_to_atoms: RefCell<HashMap<SubscriberId, HashSet<AtomId>>>,
    source_changed: Cell<bool>,
    weak_self: RefCell<Weak<RefCell<StoreInternal>>>,
    host_dirty: Rc<Cell<bool>>,
}

impl StoreInternal {
    fn new(host_dirty: Rc<Cell<bool>>) -> Self {
        StoreInternal {
            values: RefCell::new(HashMap::new()),
            closures: RefCell::new(HashMap::new()),
            derived_deps: RefCell::new(HashMap::new()),
            dependents: RefCell::new(HashMap::new()),
            stale_sources: RefCell::new(HashSet::new()),
            stale_deriveds: RefCell::new(HashSet::new()),
            next_id: Cell::new(1),
            tracker_stack: RefCell::new(Vec::new()),
            current_subscriber: Cell::new(None),
            atom_to_subs: RefCell::new(HashMap::new()),
            sub_to_atoms: RefCell::new(HashMap::new()),
            source_changed: Cell::new(false),
            weak_self: RefCell::new(Weak::new()),
            host_dirty,
        }
    }

    fn alloc_id(&self) -> AtomId {
        let id = AtomId(self.next_id.get());
        self.next_id.set(id.0 + 1);
        id
    }

    fn source<T>(&self, value: JsValue) -> Source<T> {
        let id = self.alloc_id();
        self.values.borrow_mut().insert(id, value);
        Source(id, PhantomData)
    }

    fn derive<T>(&self, closure: JsFunction) -> Derived<T> {
        let id = self.alloc_id();
        self.closures.borrow_mut().insert(id, closure);
        self.stale_deriveds.borrow_mut().insert(id);
        Derived(id, PhantomData)
    }

    fn mutate(&self, closure: JsFunction) -> Mutation {
        let id = self.alloc_id();
        self.closures.borrow_mut().insert(id, closure);
        Mutation(id)
    }

    fn subscribe(&self, atom: AtomId, sub: SubscriberId) {
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

    fn clear_subscriber(&self, sub: SubscriberId) {
        if let Some(atoms) = self.sub_to_atoms.borrow_mut().remove(&sub) {
            for atom in atoms {
                let should_remove = {
                    let mut map = self.atom_to_subs.borrow_mut();
                    if let Some(subs) = map.get_mut(&atom) {
                        subs.remove(&sub);
                        subs.is_empty()
                    } else {
                        false
                    }
                };
                if should_remove {
                    self.atom_to_subs.borrow_mut().remove(&atom);
                }
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

    fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        let id = readable.id();

        if let Readable::Derived(_) = readable {
            self.ensure_computed(id, ctx);
        }

        if let Some(top) = self.tracker_stack.borrow_mut().last_mut() {
            top.insert(id);
        }

        if let Some(sub) = self.current_subscriber.get() {
            self.subscribe(id, sub);
        }

        self.values
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or(JsValue::undefined())
    }

    fn ensure_computed(&self, id: AtomId, ctx: &mut Context) {
        if !self.stale_deriveds.borrow().contains(&id) {
            return;
        }

        let Some(closure) = self.closures.borrow().get(&id).cloned() else {
            return;
        };

        let saved_sub = self.current_subscriber.take();
        self.tracker_stack.borrow_mut().push(HashSet::new());

        let store_ctx_obj = self.build_store_ctx_obj(ctx);

        let result = closure
            .call(
                &JsValue::undefined(),
                std::slice::from_ref(&store_ctx_obj),
                ctx,
            )
            .unwrap_or_else(|_| JsValue::undefined());

        let new_deps = self.tracker_stack.borrow_mut().pop().unwrap();
        self.current_subscriber.set(saved_sub);

        if let Some(old_deps) = self.derived_deps.borrow().get(&id).cloned() {
            for dep in &old_deps {
                if let Some(set) = self.dependents.borrow_mut().get_mut(dep) {
                    set.remove(&id);
                }
            }
        }
        self.derived_deps.borrow_mut().insert(id, new_deps.clone());
        for dep in &new_deps {
            self.dependents
                .borrow_mut()
                .entry(*dep)
                .or_default()
                .insert(id);
        }

        self.values.borrow_mut().insert(id, result);
        self.stale_deriveds.borrow_mut().remove(&id);
    }

    fn build_store_ctx_obj(&self, ctx: &mut Context) -> JsValue {
        let internal = self
            .weak_self
            .borrow()
            .upgrade()
            .expect("store must be alive during recompute");
        super::build_store_context_object(ctx, Store { internal })
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined())
    }

    fn get_cached<T>(&self, readable: Readable<T>) -> JsValue {
        self.values
            .borrow()
            .get(&readable.id())
            .cloned()
            .unwrap_or(JsValue::undefined())
    }

    fn set_source<T>(&self, source: Source<T>, value: JsValue) {
        let id = source.0;
        let prev = self.values.borrow().get(&id).cloned();
        if prev.as_ref() == Some(&value) {
            return;
        }
        self.values.borrow_mut().insert(id, value);
        self.source_changed.set(true);

        let mut queue = vec![id];
        let mut visited = HashSet::new();
        while let Some(atom) = queue.pop() {
            if !visited.insert(atom) {
                continue;
            }
            if atom == id {
                self.stale_sources.borrow_mut().insert(atom);
            } else {
                self.stale_deriveds.borrow_mut().insert(atom);
            }
            if let Some(deps) = self.dependents.borrow().get(&atom).cloned() {
                for dep in deps {
                    queue.push(dep);
                }
            }
        }

        self.host_dirty.set(true);
    }

    fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        let closure = self.closures.borrow().get(&mutation.0).cloned();
        let Some(closure) = closure else {
            return Ok(JsValue::undefined());
        };
        closure.call(&JsValue::undefined(), args, ctx)
    }

    fn flush(&self) -> HashSet<AtomId> {
        if !self.source_changed.get() {
            return HashSet::new();
        }
        self.source_changed.set(false);

        let mut stale = self.stale_sources.borrow().clone();
        stale.extend(self.stale_deriveds.borrow().iter().copied());

        self.stale_sources.borrow_mut().clear();

        stale
    }

    fn has_pending(&self) -> bool {
        self.source_changed.get()
    }
}

impl std::fmt::Debug for StoreInternal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("atom_count", &self.values.borrow().len())
            .field(
                "stale_count",
                &(self.stale_sources.borrow().len() + self.stale_deriveds.borrow().len()),
            )
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Store — thin Clone wrapper.  All public methods delegate to StoreInternal
// via a short-lived `Ref` borrow.  Cheap to clone (Rc bump).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Store {
    internal: Rc<RefCell<StoreInternal>>,
}

impl Store {
    pub fn new(host_dirty: Rc<Cell<bool>>) -> Store {
        let internal = Rc::new(RefCell::new(StoreInternal::new(host_dirty)));
        *internal.borrow().weak_self.borrow_mut() = Rc::downgrade(&internal);
        Store { internal }
    }

    pub fn source<T>(&self, value: JsValue) -> Source<T> {
        self.internal.borrow().source(value)
    }

    pub fn derive<T>(&self, closure: JsFunction) -> Derived<T> {
        self.internal.borrow().derive(closure)
    }

    pub fn mutate(&self, closure: JsFunction) -> Mutation {
        self.internal.borrow().mutate(closure)
    }

    pub fn subscribe_scope(&self, sub: SubscriberId) -> SubscriberGuard {
        self.internal.borrow().current_subscriber.set(Some(sub));
        SubscriberGuard { store: self.clone() }
    }

    pub fn clear_subscriber(&self, sub: SubscriberId) {
        self.internal.borrow().clear_subscriber(sub);
    }

    pub fn dirty_subscribers(&self, atoms: &HashSet<AtomId>) -> HashSet<SubscriberId> {
        self.internal.borrow().dirty_subscribers(atoms)
    }

    pub fn read<T>(&self, readable: Readable<T>, ctx: &mut Context) -> JsValue {
        self.internal.borrow().read(readable, ctx)
    }

    pub fn get_cached<T>(&self, readable: Readable<T>) -> JsValue {
        self.internal.borrow().get_cached(readable)
    }

    pub fn set_source<T>(&self, source: Source<T>, value: JsValue) {
        self.internal.borrow().set_source(source, value);
    }

    pub fn invoke_mutation(
        &self,
        mutation: Mutation,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<JsValue> {
        self.internal.borrow().invoke_mutation(mutation, args, ctx)
    }

    pub fn flush(&self) -> HashSet<AtomId> {
        self.internal.borrow().flush()
    }

    pub fn has_pending(&self) -> bool {
        self.internal.borrow().has_pending()
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.internal.borrow().fmt(f)
    }
}

// ---------------------------------------------------------------------------
// SubscriberGuard — RAII guard that keeps an ambient subscriber active.
// Holds a Store clone (cheap); resets current_subscriber on drop.
// ---------------------------------------------------------------------------

pub struct SubscriberGuard {
    store: Store,
}

impl Drop for SubscriberGuard {
    fn drop(&mut self) {
        self.store
            .internal
            .borrow()
            .current_subscriber
            .set(None);
    }
}
