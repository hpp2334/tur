//! Module-source registry — the shared-source half of the handle-based
//! module-loading path.
//!
//! [`WorkerMsg::LoadModule`](super::WorkerMsg) carries the module source as
//! an `Arc<str>`, so a source created on the embedder's Rust side (e.g. an
//! APK asset read directly in native code, a bundle file read by a desktop
//! host) can be registered once and then loaded into any instance via
//! [`HostBackend::load_module`](crate::core::runtime::HostBackend::load_module) /
//! [`TurApp::load_module_source`](crate::TurApp::load_module_source)
//! **without ever crossing an embedder boundary as a string** — the host
//! language (Kotlin glue, JS glue, etc.) only ever sees the opaque `u64`
//! handle.
//!
//! Handles are monotonically increasing ids (never reused): a stale or
//! double-released handle is a safe miss (`get` → `None`), unlike boxed-
//! pointer handles where a dangling value would be UB. The registry is owned
//! by the embedder's runtime-side state (e.g. `AndroidRuntime` holds one and
//! clones it per instance) — it is deliberately NOT part of `TurRuntime`
//! itself, so embedders that load by string never pay for it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Shared, id-keyed store of `Arc<str>` module sources.
///
/// Cheap to clone (two `Arc`s); every clone sees the same entries. Thread-safe
/// (id allocation is atomic, the map is mutex-guarded) even though embedders
/// typically call everything from one thread — the safety net costs nothing
/// on the happy path.
#[derive(Clone, Default)]
pub struct ModuleSourceRegistry {
    inner: Arc<Mutex<HashMap<u64, Arc<str>>>>,
    next_id: Arc<AtomicU64>,
}

impl ModuleSourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `source` and return its handle. Handles start at `1` and are
    /// never reused (`0` is reserved as "no source" on the Kotlin side).
    pub fn register(&self, source: impl Into<Arc<str>>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.inner
            .lock()
            .expect("module-source registry mutex poisoned")
            .insert(id, source.into());
        id
    }

    /// Look up a source by handle. `None` for `0`, stale, or already-released
    /// handles.
    pub fn get(&self, handle: u64) -> Option<Arc<str>> {
        self.inner
            .lock()
            .expect("module-source registry mutex poisoned")
            .get(&handle)
            .cloned()
    }

    /// Release a source. Returns `true` if a source was actually removed.
    /// Idempotent for unknown handles.
    pub fn remove(&self, handle: u64) -> bool {
        self.inner
            .lock()
            .expect("module-source registry mutex poisoned")
            .remove(&handle)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_get_remove_roundtrip() {
        let reg = ModuleSourceRegistry::new();
        let id = reg.register("export function start() {}");
        assert_ne!(id, 0, "handle 0 is reserved");
        let src = reg.get(id).expect("registered source must resolve");
        assert_eq!(&*src, "export function start() {}");
        assert!(reg.remove(id));
        assert!(reg.get(id).is_none(), "released source must not resolve");
    }

    #[test]
    fn stale_and_double_release_are_safe_misses() {
        let reg = ModuleSourceRegistry::new();
        assert!(reg.get(0).is_none(), "0 is reserved");
        assert!(reg.get(u64::MAX).is_none(), "never-issued handle misses");
        let id = reg.register("x");
        assert!(reg.remove(id));
        assert!(!reg.remove(id), "double release is a no-op");
        assert!(reg.get(id).is_none());
    }

    #[test]
    fn handles_are_never_reused() {
        let reg = ModuleSourceRegistry::new();
        let a = reg.register("a");
        assert!(reg.remove(a));
        let b = reg.register("b");
        assert_ne!(a, b, "ids must not be recycled");
        assert_eq!(&*reg.get(b).unwrap(), "b");
    }

    #[test]
    fn clones_share_entries() {
        let reg = ModuleSourceRegistry::new();
        let clone = reg.clone();
        let id = reg.register("shared");
        assert!(clone.get(id).is_some());
        assert!(clone.remove(id));
        assert!(reg.get(id).is_none());
    }
}
