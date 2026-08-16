//! Worker pool declarations.
//!
//! A [`WorkerPoolHandle`] is an inert, cheap-to-clone declaration of a
//! named pool with a maximum worker count. It carries no runtime state:
//! it becomes live only after being registered on the runtime builder
//! ([`TurRuntimeBuilder::worker_pool`]) and is then assigned per-app via
//! [`TurAppBuilder::worker_pool`]. All apps assigned to the same pool
//! share at most `max_workers` workers: the platform
//! [`WorkerSpawner`](crate::core::scheduler::WorkerSpawner) picks or creates a
//! worker for each app — first come, first served up to the cap, then
//! apps share the least-loaded worker cooperatively.
//!
//! Pool identity is the `Arc` behind the handle ([`WorkerPoolHandle::ptr_eq`]),
//! so two handles built with identical `name`/`max_workers` are still
//! distinct pools. The runtime builder rejects duplicate names so identity
//! and name never diverge.
//!
//! A cap greater than or equal to the app count degenerates to
//! one-worker-per-app (the historical default): every app gets a fresh
//! worker until the cap is reached.
//!
//! [`TurRuntimeBuilder::worker_pool`]: crate::TurRuntimeBuilder::worker_pool
//! [`TurAppBuilder::worker_pool`]: crate::TurAppBuilder::worker_pool

use std::sync::Arc;

/// Declaration of a named worker pool with a capped worker count.
///
/// Built via [`WorkerPoolHandle::new`], registered once on the runtime
/// builder, and assigned to each app builder. See the [module docs](self)
/// for the sharing model.
#[derive(Clone)]
pub struct WorkerPoolHandle(Arc<PoolDef>);

pub(crate) struct PoolDef {
    name: String,
    max_workers: usize,
}

impl std::fmt::Debug for WorkerPoolHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerPoolHandle")
            .field("name", &self.0.name)
            .field("max_workers", &self.0.max_workers)
            .finish()
    }
}

impl WorkerPoolHandle {
    /// Declare a pool. `max_workers` caps how many workers the pool
    /// ever runs concurrently; apps beyond the cap share the existing
    /// workers cooperatively. Must be `>= 1` (validated when the pool is
    /// registered on the runtime builder). `usize::MAX` restores the
    /// historical one-worker-per-app behavior.
    pub fn new(name: impl Into<String>, max_workers: usize) -> Self {
        Self(Arc::new(PoolDef {
            name: name.into(),
            max_workers,
        }))
    }

    /// The pool's name (unique per runtime).
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// The maximum number of workers (OS threads on native, Web Workers on wasm) the pool may run concurrently.
    pub fn max_workers(&self) -> usize {
        self.0.max_workers
    }

    /// Identity check — two handles are the same pool iff they point at the
    /// same declaration (`Arc` pointer equality). Distinct handles built
    /// with identical `new(name, max)` arguments are still distinct pools.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
