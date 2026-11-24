//! The pkg-zip module carrier — a packed SOURCE ZIP (`rut pack --mode
//! source`) as a loadable rut module: `rut.jsonc` + the `src/` tree +
//! the embedded `deps/<identity>/` closure, unpacked once (memory) and
//! mounted per load.
//!
//! This is the pkg-tree load lane beside [`RutModuleFiles`]: a file set
//! mounts a bare module tree (the engine synthesizes the app manifest
//! over its offered set); a pkg zip mounts a real PACKAGE — the zip's
//! own `rut.jsonc` names the closure the archive carries, every embedded
//! group mounts from its own subtree, and the session (the offered set)
//! stays first-mount-wins. Nameability follows the engine's ambient law
//! (the app manifest names the offered specs ∪ the zip's dep heads), so
//! a plugin entrypoint's `rut.jsonc` only ever declares what it EMBEDS
//! (path deps); session deps (`emp`, `json`, the host pkgs) resolve
//! against the offered set exactly like a file-set load does.
//!
//! Only TOP-LEVEL groups mount (`deps/<identity>/…` with a single-segment
//! identity). A group's own nested `deps/` subtree rides as inert bytes —
//! its specs resolve from the session, whose mounts precede the groups
//! (first-mount-wins), so the packed closure duplicates only what the
//! embedder did not offer.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rut_driver::bundle::{
    is_identity, mount_name, parse_manifest, read_srczip, PkgType, MANIFEST_NAME,
};
use rut_driver::{Mounted, MountedBody, Mode};

use super::RutModuleFiles;

/// One unpacked source zip: the entry list `read_srczip` validated
/// (exactly one `rut.jsonc` at the archive root, the `src/`-shell +
/// `deps/<identity>/` placement grammar). Cheap to clone (one Arc);
/// the bytes flow to the worker by refcount — the archive never
/// re-unpacks per load.
#[derive(Clone, Debug)]
pub struct RutModulePkgZip {
    pub entries: Arc<[(String, Vec<u8>)]>,
}

impl RutModulePkgZip {
    /// Unpack + validate packed source-zip bytes (the one unpack — the
    /// registry lane and the direct load lane both land here).
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let entries = read_srczip(data).map_err(|e| e.to_string())?;
        Ok(Self {
            entries: entries.into(),
        })
    }

    /// The archive root manifest's raw text (the pack rewrote its dep
    /// rows to point inside at `deps/<identity>/`).
    pub fn manifest_text(&self) -> Result<String, String> {
        self.entries
            .iter()
            .find(|(n, _)| n == MANIFEST_NAME)
            .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
            .ok_or_else(|| format!("the pkg zip carries no {MANIFEST_NAME} at the archive root"))
    }

    /// Total carried bytes (logging/diagnostics).
    pub fn total_len(&self) -> usize {
        self.entries.iter().map(|(_, b)| b.len()).sum()
    }
}

/// Collect the top-level embedded groups (single-segment identities):
/// ident → the subtree prefix (`deps/<ident>/`), deterministically by
/// ident. A nested `deps/` inside a group's subtree is NOT a top-level
/// group — its prefix carries two identities and never matches here.
fn groups_of(files: &RutModulePkgZip) -> BTreeMap<&str, String> {
    let mut out: BTreeMap<&str, String> = BTreeMap::new();
    for (name, _) in files.entries.iter() {
        let rest = match name.strip_prefix("deps/") {
            Some(r) => r,
            None => continue,
        };
        let (ident, tail) = match rest.split_once('/') {
            Some((i, t)) => (i, t),
            None => continue,
        };
        if !is_identity(ident) || tail.is_empty() {
            continue;
        }
        out.entry(ident)
            .or_insert_with(|| format!("deps/{ident}/"));
    }
    out
}

/// The `src/`-shell file set of one package subtree: the root module
/// (`src/mod.rut`) plus its declared children keyed by mod path
/// (`a`, `a/b`, single-file `a.rut` → `a`). Returns `(root, rows)`.
fn src_tree(
    entries: &[(String, Vec<u8>)],
    prefix: &str,
    label: &str,
) -> Result<(String, BTreeMap<String, String>), String> {
    let src = format!("{prefix}src/");
    let mut root: Option<String> = None;
    let mut rows: BTreeMap<String, String> = BTreeMap::new();
    for (name, bytes) in entries {
        let rel = match name.strip_prefix(&src) {
            Some(r) => r,
            None => continue,
        };
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| format!("{label}: {name} is not UTF-8"))?;
        if rel == "mod.rut" {
            root = Some(text);
            continue;
        }
        // the mod path a `src/`-relative file mounts as — the pack
        // lane's own derivation (`a/mod.rut` → `a`, `a.rut` → `a`)
        let path = rel
            .strip_suffix("/mod.rut")
            .or_else(|| rel.strip_suffix(".rut"))
            .unwrap_or(rel);
        rows.insert(path.to_string(), text);
    }
    let root = root.ok_or_else(|| format!("{label}: no src/mod.rut — the pkg zip has no root module"))?;
    Ok((root, rows))
}

/// Mount one package subtree as a source [`Mounted`]: manifest parsed,
/// `src/` tree mounted by declaration, spec stamped (a drifted manifest
/// name is the loud mismatch).
fn mount_source_pkg(
    entries: &[(String, Vec<u8>)],
    prefix: &str,
    manifest_text: &str,
    spec: &str,
    label: &str,
) -> Result<Mounted, String> {
    let (root, rows) = src_tree(entries, prefix, label)?;
    let mods = rut_driver::mods::mount_rows(&root, &rows, Mode::Impl, "mod.rut")
        .map_err(|e| format!("{label}: {e}"))?;
    let mut m = parse_manifest(manifest_text).map_err(|e| format!("{label}: rut.jsonc: {e}"))?;
    // The mount spec is OURS (the group's identity name half): the
    // packed manifest already names this pkg — a mismatch is a broken
    // archive, never a silent rename.
    if m.name.as_deref().is_some_and(|n| n != spec) {
        return Err(format!(
            "{label}: rut.jsonc names itself `{}` — expected `{spec}`",
            m.name.as_deref().unwrap_or("<unnamed>")
        ));
    }
    m.name = Some(spec.to_string());
    Ok(Mounted {
        spec: spec.to_string(),
        manifest: m,
        body: MountedBody::Source {
            text: root,
            is_decl: false,
        },
        mods,
        ..Default::default()
    })
}

/// Mount a packed source zip: the archive-root package (its manifest's
/// declaration UNIONED with `offered_specs` — the engine's ambient
/// nameability law, the file-set lane's exact shape) plus every
/// top-level embedded group whose spec the session does not already
/// mount. The caller mounts the returned app FIRST, then the offered
/// set, then the groups — first-mount-wins keeps the embedder's set
/// ahead of the packed closure.
pub(crate) fn mount_pkg_zip(
    files: &RutModulePkgZip,
    offered_specs: &BTreeSet<String>,
) -> Result<(Mounted, Vec<Mounted>), String> {
    let manifest_text = files.manifest_text()?;
    let manifest =
        parse_manifest(&manifest_text).map_err(|e| format!("pkg zip rut.jsonc: {e}"))?;
    let spec = manifest
        .name
        .clone()
        .ok_or_else(|| "pkg zip rut.jsonc has no `name`".to_string())?;
    // The app's declaration: the zip's own dep heads ∪ the offered
    // specs (ambient nameability — the wasm-playground law the file
    // lane runs). The zip's dep heads mount from the archive below;
    // every other declared head resolves from the session.
    let mut heads: BTreeSet<String> = offered_specs.clone();
    heads.extend(manifest.deps.keys().cloned());
    let head_list: Vec<&str> = heads.iter().map(|s| s.as_str()).collect();
    let app_manifest = rut_driver::Manifest::lib(&spec, &head_list);
    let (root, rows) = src_tree(&files.entries, "", "pkg zip")?;
    let mods = rut_driver::mods::mount_rows(&root, &rows, Mode::Impl, "mod.rut")
        .map_err(|e| format!("pkg zip: {e}"))?;
    let app = Mounted {
        spec: spec.clone(),
        manifest: app_manifest,
        body: MountedBody::Source {
            text: root,
            is_decl: false,
        },
        mods,
        ..Default::default()
    };

    // The embedded closure: top-level groups only; a spec the session
    // already mounts (or the root itself) is skipped — first-mount-wins.
    let mut extra = Vec::new();
    for (ident, prefix) in groups_of(files) {
        let group_spec = mount_name(ident).to_string();
        if group_spec == spec || offered_specs.contains(&group_spec) {
            continue;
        }
        let group_manifest = String::from_utf8(
            files
                .entries
                .iter()
                .find(|(n, _)| n == &format!("{prefix}{MANIFEST_NAME}"))
                .map(|(_, b)| b.clone())
                .ok_or_else(|| format!("deps/{ident}: no {prefix}{MANIFEST_NAME}"))?,
        )
        .map_err(|_| format!("deps/{ident}: rut.jsonc is not UTF-8"))?;
        let kind = parse_manifest(&group_manifest)
            .map_err(|e| format!("deps/{ident}: rut.jsonc: {e}"))?
            .pkg_type;
        let label = format!("deps/{group_spec}");
        let mounted = if kind == PkgType::Host {
            // A host pkg travels as its declaration surface — the
            // decl-lane mount (rows bind at boot from the session's own
            // install; the surface type-checks against them).
            let (root, rows) = decl_tree(&files.entries, &prefix, &label)?;
            let mods =
                rut_driver::mods::mount_rows(&root, &rows, Mode::Decl, "mod.d.rut")
                    .map_err(|e| format!("{label}: {e}"))?;
            let mut m = rut_driver::lower_decl_module(&root, &mods, &label)
                .map_err(|e| format!("{label}: {e}"))?;
            m.spec = group_spec;
            m
        } else {
            mount_source_pkg(&files.entries, &prefix, &group_manifest, &group_spec, &label)?
        };
        extra.push(mounted);
    }
    Ok((app, extra))
}

/// The `src/`-shell declaration tree of a HOST subtree: root surface
/// `src/mod.d.rut` + its `mod NAME;` children (`NAME/mod.d.rut` /
/// `NAME.rut`), keyed by mod path.
fn decl_tree(
    entries: &[(String, Vec<u8>)],
    prefix: &str,
    label: &str,
) -> Result<(String, BTreeMap<String, String>), String> {
    let src = format!("{prefix}src/");
    let mut root: Option<String> = None;
    let mut rows: BTreeMap<String, String> = BTreeMap::new();
    for (name, bytes) in entries {
        let rel = match name.strip_prefix(&src) {
            Some(r) => r,
            None => continue,
        };
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| format!("{label}: {name} is not UTF-8"))?;
        if rel == "mod.d.rut" {
            root = Some(text);
            continue;
        }
        let path = rel
            .strip_suffix("/mod.d.rut")
            .or_else(|| rel.strip_suffix(".d.rut"))
            .unwrap_or(rel);
        rows.insert(path.to_string(), text);
    }
    let root = root.ok_or_else(|| {
        format!("{label}: no src/mod.d.rut — a host group needs its declaration root")
    })?;
    Ok((root, rows))
}

/// The diag-rendering label for a pkg-zip load: the ROOT package's own
/// file set (the same shape [`render_diags`] attributes spans against —
/// offered/kit diags bucket exactly as the file lane's do).
pub(crate) fn root_files(files: &RutModulePkgZip) -> Result<RutModuleFiles, String> {
    let (root, rows) = src_tree(&files.entries, "", "pkg zip")?;
    Ok(RutModuleFiles {
        root: root.into(),
        mods: rows
            .into_iter()
            .map(|(p, t)| (Arc::from(p.as_str()), Arc::from(t.as_str())))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rut_driver::bundle::write_srczip;

    fn zip_bytes(entries: Vec<(&str, &str)>) -> Vec<u8> {
        let owned: Vec<(String, Vec<u8>)> = entries
            .into_iter()
            .map(|(n, b)| (n.to_string(), b.as_bytes().to_vec()))
            .collect();
        write_srczip(&owned).expect("test zip writes")
    }

    #[test]
    fn from_bytes_roundtrips_and_refuses_junk() {
        let bytes = zip_bytes(vec![("rut.jsonc", "{\"name\":\"m\"}"), ("src/mod.rut", "entry fn start() {}")]);
        let z = RutModulePkgZip::from_bytes(&bytes).expect("valid zip");
        assert_eq!(z.manifest_text().unwrap(), "{\"name\":\"m\"}");
        assert!(RutModulePkgZip::from_bytes(b"not a zip").is_err());
        // (the placement-grammar refusals are the writer's law too —
        // `write_srczip` never emits what the reader would refuse — so a
        // bad-archive case cannot even be constructed through the public
        // API here; upstream's srczip tests cover that door.)
    }

    #[test]
    fn groups_of_enumerates_only_top_level_identities() {
        let bytes = zip_bytes(vec![
            ("rut.jsonc", "{\"name\":\"m\"}"),
            ("src/mod.rut", "entry fn start() {}"),
            ("deps/crypto@aaaaaaaaaaaaaaaa/deps/crypto_host@bbbbbbbbbbbbbbbb/rut.jsonc", "{\"name\":\"crypto_host\"}"),
        ]);
        let z = RutModulePkgZip::from_bytes(&bytes).unwrap();
        let groups = groups_of(&z);
        assert_eq!(groups.len(), 1, "only the top-level identity mounts");
        assert_eq!(mount_name(*groups.keys().next().unwrap()), "crypto");
    }
}
