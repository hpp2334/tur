//! `watch(readable, cb)` — non-element subscribers over the reactive store.
//!
//! A watcher is a registered edge (watched atom → callback mutation) plus a
//! control pair (`start$` / `stop$` mutations). While started, the flush loop
//! delivers the callback whenever the watched atom is dirtied — queued onto
//! the mutation queue, so invocation goes through the exact same rail as
//! every other mutation (mounted-store ctx, same fixed-point iteration).
//!
//! Two loop guards keep the fixed-point loop convergent:
//! 1. **Reentrancy check** (`SharedReactive::detect_watch_loop`, run from
//!    `write_by_id`): while a watcher callback is being invoked, a write that
//!    transitively invalidates a delivering watcher's watched atom throws a
//!    JS error at the call site. Catches the direct case — a callback
//!    re-invalidating what it watches.
//! 2. **Epoch coalescing** (`take_due`): each watcher delivers at most once
//!    per flush epoch (`frame_id`). Catches indirect ping-pong (W1 → W2 → W1)
//!    that sequential — not nested — invocation would otherwise spin forever.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::core::edgy::reactive::{AtomId, Mutation};

/// Unique watcher id, allocated from the registry's own counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WatcherId(u32);

struct WatcherEntry {
    watched: AtomId,
    callback: Mutation,
    active: bool,
    /// Last flush epoch this watcher was delivered at (u64::MAX never
    /// matches a real epoch — a fresh watcher is always due).
    last_epoch: u64,
}

/// Registry of all `watch()` declarations of one instance. Lives on
/// [`SharedReactive`](crate::core::edgy::reactive::SharedReactive) next to
/// the subscriber graph — same interior-mutability discipline, `&self`
/// methods only.
pub(crate) struct WatcherRegistry {
    next: Cell<u32>,
    entries: RefCell<HashMap<WatcherId, WatcherEntry>>,
    /// watched atom → active watcher ids (the delivery index; edges exist
    /// only while started, so stopped watchers cost nothing).
    index: RefCell<HashMap<AtomId, HashSet<WatcherId>>>,
    /// callback mutation id → watchers (arms the delivery guard when the
    /// engine invokes that mutation). A Vec because the same mutation may
    /// be shared by several `watch()` declarations.
    by_callback: RefCell<HashMap<AtomId, Vec<WatcherId>>>,
    /// Stack of per-invocation watcher sets whose callbacks are currently
    /// being invoked (non-empty ⇒ `write_by_id` runs loop detection).
    delivering: RefCell<Vec<Vec<WatcherId>>>,
}

impl Default for WatcherRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WatcherRegistry {
    pub(crate) fn new() -> Self {
        WatcherRegistry {
            next: Cell::new(1),
            entries: RefCell::new(HashMap::new()),
            index: RefCell::new(HashMap::new()),
            by_callback: RefCell::new(HashMap::new()),
            delivering: RefCell::new(Vec::new()),
        }
    }

    /// Register a (dormant) watcher. `activate` adds the delivery edge.
    pub(crate) fn register(&self, watched: AtomId, callback: Mutation) -> WatcherId {
        let id = WatcherId(self.next.get());
        self.next.set(id.0 + 1);
        self.by_callback
            .borrow_mut()
            .entry(callback.id())
            .or_default()
            .push(id);
        self.entries.borrow_mut().insert(
            id,
            WatcherEntry {
                watched,
                callback,
                active: false,
                last_epoch: u64::MAX,
            },
        );
        id
    }

    /// Start delivering: add the index edge. Idempotent. Returns the watched
    /// atom + callback (the caller materializes the watched atom once — see
    /// `ReactiveBridgeStore::register_watch`).
    pub(crate) fn activate(&self, id: WatcherId) -> Option<(AtomId, Mutation)> {
        let watched = {
            let mut entries = self.entries.borrow_mut();
            let entry = entries.get_mut(&id)?;
            entry.active = true;
            entry.watched
        };
        self.index
            .borrow_mut()
            .entry(watched)
            .or_default()
            .insert(id);
        let callback = self.entries.borrow().get(&id).map(|e| e.callback)?;
        Some((watched, callback))
    }

    /// Stop delivering (drop the index edge). Idempotent; the entry stays
    /// registered so a later `start$` resumes the same watcher.
    pub(crate) fn deactivate(&self, id: WatcherId) {
        let watched = {
            let mut entries = self.entries.borrow_mut();
            let Some(entry) = entries.get_mut(&id) else {
                return;
            };
            entry.active = false;
            entry.watched
        };
        let mut index = self.index.borrow_mut();
        if let Some(subs) = index.get_mut(&watched) {
            subs.remove(&id);
            if subs.is_empty() {
                index.remove(&watched);
            }
        }
    }

    /// Callbacks due for a set of dirtied atoms: active watchers whose
    /// watched atom is dirtied, at most once per epoch. Stamps the epoch so
    /// the same watcher never delivers twice in one flush epoch (the
    /// convergence backstop for indirect write cycles).
    pub(crate) fn take_due(&self, dirties: &HashSet<AtomId>, epoch: u64) -> Vec<Mutation> {
        let mut out = Vec::new();
        let mut entries = self.entries.borrow_mut();
        for atom in dirties {
            let Some(subs) = self.index.borrow().get(atom).cloned() else {
                continue;
            };
            for id in subs {
                let Some(entry) = entries.get_mut(&id) else {
                    continue;
                };
                if !entry.active || entry.last_epoch == epoch {
                    continue;
                }
                entry.last_epoch = epoch;
                out.push(entry.callback);
            }
        }
        out
    }

    /// Arm the delivery guard if `mutation` is a registered watcher callback
    /// (possibly of several watchers — a mutation can be shared). Returns
    /// whether the caller must `disarm`.
    pub(crate) fn arm(&self, callback_id: AtomId) -> bool {
        let Some(ids) = self.by_callback.borrow().get(&callback_id).cloned() else {
            return false;
        };
        if ids.is_empty() {
            return false;
        }
        self.delivering.borrow_mut().push(ids);
        true
    }

    pub(crate) fn disarm(&self) {
        self.delivering.borrow_mut().pop();
    }

    pub(crate) fn is_delivering(&self) -> bool {
        !self.delivering.borrow().is_empty()
    }

    /// The watched atoms of every watcher currently delivering (the guard's
    /// protected set).
    pub(crate) fn delivering_watched(&self) -> Vec<AtomId> {
        let delivering = self.delivering.borrow();
        let entries = self.entries.borrow();
        delivering
            .iter()
            .flatten()
            .filter_map(|id| entries.get(id).map(|e| e.watched))
            .collect()
    }
}
