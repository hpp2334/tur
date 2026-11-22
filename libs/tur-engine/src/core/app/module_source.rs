//! Module-source registry — the shared-source half of the handle-based
//! module-loading path.
//!
//! [`WorkerMsg::LoadModule`](super::WorkerMsg) carries the module source as
//! an `Arc<str>`, so a source created on the embedder's Rust side (e.g. an
//! APK asset read directly in native code, a bundle file read by a desktop
//! host) can be registered once and then loaded into any instance via
//! [`TurApp::load_module_source`](crate::TurApp::load_rut_module_source)
//! (or its string-based sibling
//! [`TurApp::load_module`](crate::TurApp::load_rut_module))
//! **without ever crossing an embedder boundary as a string** — the host
//! language (Kotlin glue, JS glue, etc.) only ever sees the opaque `u64`
//! handle. Entries are either a single source ([`ModuleSourceEntry::Source`])
//! or a whole FILE SET ([`ModuleSourceEntry::Files`] — `register_files`,
//! the multi-file module carrier); a file-set handle loads through the same
//! multi-file rail an explicit `RutModuleFiles` load rides.
//!
//! Handles are monotonically increasing ids (never reused): a stale or
//! double-released handle is a safe miss (`get` → `None`), unlike boxed-
//! pointer handles where a dangling value would be UB. The registry is owned
//! by the embedder's runtime-side state (e.g. `AndroidRuntime` holds one and
//! clones it per instance) — it is deliberately NOT part of `TurRuntime`
//! itself, so embedders that load by string never pay for it.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::rut_runtime::RutModuleFiles;

/// A registered module source — the registry's two shapes.
#[derive(Clone, Debug)]
pub enum ModuleSourceEntry {
    /// The historical single-source shape (the root file only).
    Source(Arc<str>),
    /// A multi-file module: the root source plus its declared `mod`
    /// children. `TurApp::load_rut_module_source` resolves it whole
    /// through the engine's files rail.
    Files(RutModuleFiles),
}

/// Shared, id-keyed store of module sources (single sources and file
/// sets).
///
/// Cheap to clone (two `Arc`s); every clone sees the same entries. Thread-safe
/// (id allocation is atomic, the map is mutex-guarded) even though embedders
/// typically call everything from one thread — the safety net costs nothing
/// on the happy path.
#[derive(Clone, Default)]
pub struct ModuleSourceRegistry {
    inner: Arc<Mutex<HashMap<u64, ModuleSourceEntry>>>,
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
            .insert(id, ModuleSourceEntry::Source(source.into()));
        id
    }

    /// Register a whole FILE SET — `root` plus its declared `mod` children
    /// (`mod path → child text`, the [`RutModuleFiles`] grammar) — and
    /// return its handle. The set loads whole through the multi-file rail
    /// (`TurApp::load_rut_module_source` dispatches [`ModuleSourceEntry::Files`]
    /// to `load_rut_module_files`); the child texts never cross an embedder
    /// boundary as strings.
    pub fn register_files(
        &self,
        root: impl Into<Arc<str>>,
        mods: BTreeMap<Arc<str>, Arc<str>>,
    ) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.inner
            .lock()
            .expect("module-source registry mutex poisoned")
            .insert(
                id,
                ModuleSourceEntry::Files(RutModuleFiles {
                    root: root.into(),
                    mods,
                }),
            );
        id
    }

    /// Look up a source by handle. `None` for `0`, stale, or already-released
    /// handles.
    pub fn get(&self, handle: u64) -> Option<ModuleSourceEntry> {
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
        let id = reg.register("entry fn start() {}");
        assert_ne!(id, 0, "handle 0 is reserved");
        let src = reg.get(id).expect("registered source must resolve");
        assert!(
            matches!(&src, ModuleSourceEntry::Source(s) if &**s == "entry fn start() {}"),
            "register spells the Source shape"
        );
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
        assert!(matches!(&reg.get(b).unwrap(), ModuleSourceEntry::Source(s) if &**s == "b"));
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

    #[test]
    fn register_files_roundtrips_the_whole_set() {
        let reg = ModuleSourceRegistry::new();
        let mut mods = BTreeMap::new();
        mods.insert(Arc::from("child"), Arc::from("pub fn two() -> i32 { 2 }"));
        let id = reg.register_files("mod child;\nentry fn start() {}", mods);
        let entry = reg.get(id).expect("registered file set must resolve");
        let ModuleSourceEntry::Files(files) = entry else {
            panic!("register_files spells the Files shape");
        };
        assert_eq!(&*files.root, "mod child;\nentry fn start() {}");
        assert_eq!(files.mods.len(), 1);
        assert_eq!(&*files.mods["child"], "pub fn two() -> i32 { 2 }");
        assert!(reg.remove(id));
        assert!(reg.get(id).is_none());
    }
}
