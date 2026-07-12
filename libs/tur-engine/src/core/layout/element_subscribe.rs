use std::collections::HashSet;

use crate::core::reactive::{AnyReadable, SubscriberId, SubscriberIndexStore};
use crate::core::view::{FromJs, Val};

// ---------------------------------------------------------------------------
// ElementSubscribe — explicit declaration of which reactive atoms a node
// depends on, so a reactive flush can mark the node (and, via `mark_dirty`'s
// ancestor walk, its parents) dirty for re-layout.
//
// Runs as a dedicated phase after `perform_layout` for every dirty node. Each
// element lists its own `Val::Reactive` props via `cx.subscribe_val`; the
// collected set atomically replaces the node's prior subscriptions. Default
// impl is a no-op so elements without reactive props need no body.
//
// This replaces the previous ambient auto-tracking where `LayoutContext::read_val`
// registered subscriptions as a side-effect (via `subscribe_scope`).
// ---------------------------------------------------------------------------

pub trait ElementSubscribe {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let _ = cx;
    }
}

// ---------------------------------------------------------------------------
// SubscribeCx — accumulator for one node's declared atom deps during a single
// subscribe pass. Holds a `SubscriberIndexStore` capability (the write face over the
// store's atom→subscriber index) so the `Drop` impl can swap the collected set
// in without entangling layout borrows.
// ---------------------------------------------------------------------------

pub struct SubscribeCx {
    store: SubscriberIndexStore,
    node: SubscriberId,
    new_deps: HashSet<AnyReadable>,
}

impl SubscribeCx {
    pub fn new(store: SubscriberIndexStore, node: SubscriberId) -> Self {
        SubscribeCx {
            store,
            node,
            new_deps: HashSet::new(),
        }
    }

    /// Declare a reactive dependency for the current node. No-op for static
    /// vals. The element unwraps `Option<Val<T>>` itself before calling this.
    pub fn subscribe_val<T: FromJs + Clone + 'static>(&mut self, val: &Val<T>) {
        if let Some(atom) = val.atom() {
            self.new_deps.insert(atom);
        }
    }

    /// Declare a raw atom dependency (for fragments that hold an `AnyReadable`
    /// rather than a `Val<T>`).
    pub fn subscribe_readable(&mut self, atom: AnyReadable) {
        self.new_deps.insert(atom);
    }
}

impl Drop for SubscribeCx {
    fn drop(&mut self) {
        let deps = std::mem::take(&mut self.new_deps);
        self.store.set_subscriber_deps(self.node, deps);
    }
}
