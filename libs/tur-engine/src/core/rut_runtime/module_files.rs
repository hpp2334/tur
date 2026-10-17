//! The multi-file module carrier — a rut module's file set as data.
//!
//! A module is a TREE of files: the root source plus the mod children its
//! `mod NAME;` declarations name. [`RutModuleFiles`] carries that set so it
//! can ride the rails (the worker RPC, the virtual-app child records)
//! as one clone-cheap value. Single-string loads are the root-only shape —
//! `From<&str>` / `From<Arc<str>>` / `From<String>` build them — so every
//! existing call site keeps working unchanged.

use std::collections::BTreeMap;
use std::sync::Arc;

/// A rut module's file set: the root source plus its mounted mod children.
///
/// `mods` is keyed by mod path ("landing", "net/tls" — the declaration
/// grammar's paths) → the child file's text. The BTreeMap is load-bearing:
/// its path order is the concatenated unit's byte layout (see
/// `GraphCompiler::ensure_source` upstream), which the diag attribution
/// mirrors.
#[derive(Clone, Debug, Default)]
pub struct RutModuleFiles {
    /// The root module's source (`mod.rut` in the on-disk spelling).
    pub root: Arc<str>,
    /// mod path ("landing") → text — declaration-driven at mount time (a
    /// carried-but-undeclared row is refused by `mount_rows`).
    pub mods: BTreeMap<Arc<str>, Arc<str>>,
}

impl From<&str> for RutModuleFiles {
    fn from(root: &str) -> Self {
        Self {
            root: Arc::from(root),
            mods: BTreeMap::new(),
        }
    }
}

impl From<String> for RutModuleFiles {
    fn from(root: String) -> Self {
        Self {
            root: Arc::from(root.as_str()),
            mods: BTreeMap::new(),
        }
    }
}

impl From<Arc<str>> for RutModuleFiles {
    fn from(root: Arc<str>) -> Self {
        Self {
            root,
            mods: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_is_root_only() {
        let f = RutModuleFiles::from("entry fn start() {}");
        assert_eq!(f.root.as_ref(), "entry fn start() {}");
        assert!(f.mods.is_empty());
    }

    #[test]
    fn mods_iterate_in_path_order() {
        let mut f = RutModuleFiles::from("mod b;\nmod a;\n");
        f.mods.insert(
            Arc::from("b"),
            Arc::from("pub fn two() -> i32 { return 2; }"),
        );
        f.mods.insert(
            Arc::from("a"),
            Arc::from("pub fn one() -> i32 { return 1; }"),
        );
        let paths: Vec<&str> = f.mods.keys().map(|p| p.as_ref()).collect();
        assert_eq!(paths, vec!["a", "b"], "BTreeMap = path order (the concat law)");
    }

    #[test]
    fn clone_is_cheap_and_sharing() {
        let mut f = RutModuleFiles::from("mod a;\n");
        f.mods.insert(Arc::from("a"), Arc::from("pub fn x() {}"));
        let g = f.clone();
        assert!(Arc::ptr_eq(&f.root, &g.root));
        assert!(Arc::ptr_eq(f.mods.get("a").unwrap(), g.mods.get("a").unwrap()));
    }
}
