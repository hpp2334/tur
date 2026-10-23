//! The `tur_host` decl-TREE pin + the kit tree pin.
//!
//! Three artifacts are pinned here against the LIVE engine session:
//!
//! 1. `rut/tur_host/` — the GENERATED decl MODULE TREE of the standard
//!    session's merged `tur_host` surface (`tur_decl_pkg()` + every
//!    plugin's RutPkgExt rows): a `mod.d.rut` root (the auto-discovered
//!    default root beside the manifest — the manifest spells no
//!    `entry`) plus one `NAME/mod.d.rut` per module, recursive
//!    (`layout/flex/mod.d.rut` nests under `layout`). The Rust rows
//!    stay the compile truth; this test rebuilds the file set and diffs
//!    it — drift fails the suite until the tree is deliberately
//!    regenerated (bless mode: `TUR_BLESS_TUR_HOST_DECL=1`). The
//!    committed tree must also load through the driver's REAL discovery
//!    lane with a row set EXACTLY the live one (paths included).
//! 2. `rut/tur_kit/rut.jsonc` — the kit manifest parses with NO repealed
//!    entry keys (the root module is the walk's `mod.rut`), and the
//!    kit's FILE-MODULE TREE on disk (every directory's `mod.rut`) is
//!    byte-identical to the engine's embedded `Mounted.mods` tree.
//! 3. `rut/tur_host/rut.jsonc` — the host pkg's manifest must declare
//!    `type = "host"` and NO `entry` (the `mod.d.rut` root is the
//!    default).

use std::path::{Path, PathBuf};

use rut_driver::MountedBody;
use rut_driver::bundle::{PkgType, parse_manifest};
use tur_animation::TurAnimationPlugin;
use tur_engine::core::capability::Capabilities;
use tur_engine::core::plugin::Plugin;
use tur_engine::core::runtime::probe_register_rut_pkg_exts;
use tur_engine::core::rut_runtime::{render_tur_host_decl_tree, tur_host_surface};
use tur_engine::{TurClipboardPlugin, TurFontPlugin, TurStdPlugin};
use tur_integration_tests::{RecordingClipboard, RecordingHttp};
use tur_net_capability::TurNetPlugin;

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// The standard session's plugin set — the SAME set the corpus compile
/// gate builds (`TurTestApp::new_with_http`): std + animation + clipboard
/// + net + fonts (the Http + Fonts capabilities present, so the net and
/// font rows install).
fn standard_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(TurStdPlugin),
        Box::new(TurAnimationPlugin),
        Box::new(TurClipboardPlugin),
        Box::new(TurNetPlugin),
        Box::new(TurFontPlugin),
    ]
}

/// The standard session's capabilities: the recording clipboard + a fresh
/// recording http (no requests are ever made — the backends only need to
/// satisfy the net/font plugins' presence checks).
fn standard_capabilities() -> Capabilities {
    let caps = Capabilities::new();
    caps.insert(tur_engine::Clipboard::new(RecordingClipboard::new()));
    caps.insert(tur_net_capability::Http::new(RecordingHttp::new()));
    caps.insert(
        tur_engine::Fonts::new(RecordingFontProvider),
    );
    caps
}

/// The decl probe's font provider — never driven (the backends only need
/// to satisfy `TurFontPlugin`'s presence check).
struct RecordingFontProvider;

impl tur_engine::FontDataProvider for RecordingFontProvider {
    fn load(
        &self,
        _meta: &tur_engine::core::fonts::FontFaceMeta,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<u8>, tur_engine::error::TurError>> + '_>,
    > {
        Box::pin(std::future::ready(Ok(Vec::new())))
    }
}

fn probe_exts() -> Vec<tur_engine::core::rut_runtime::RutPkgExt> {
    probe_register_rut_pkg_exts(&standard_plugins(), standard_capabilities(), (400.0, 600.0))
        .expect("the standard session's register probe")
}

#[test]
fn tur_host_decl_tree_matches_the_standard_session() {
    let live = render_tur_host_decl_tree(&probe_exts());
    assert!(
        live.len() > 10,
        "the tree renderer answered {} files — the pkg is a MODULE TREE",
        live.len()
    );
    let pkg_dir = workspace_root().join("rut/tur_host");
    if std::env::var("TUR_BLESS_TUR_HOST_DECL").is_ok() {
        for (rel, text) in &live {
            let path = pkg_dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).expect("bless: mkdir");
            std::fs::write(&path, text).expect("bless: write the module file");
            eprintln!("blessed {}", path.display());
        }
        // Stale generated files out: any `.d.rut` NOT in the current set
        // goes (the old flat `tur_host.d.rut`, a renamed module's file).
        // rut.jsonc is never touched.
        let live_set: std::collections::BTreeSet<String> =
            live.iter().map(|(rel, _)| rel.clone()).collect();
        fn walk(
            dir: &Path,
            rel: &str,
            live_set: &std::collections::BTreeSet<String>,
            stale: &mut Vec<PathBuf>,
        ) {
            for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let child_rel = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                if entry.path().is_dir() {
                    walk(&entry.path(), &child_rel, live_set, stale);
                } else if name.ends_with(".d.rut") && !live_set.contains(&child_rel) {
                    stale.push(entry.path());
                }
            }
        }
        let mut stale = Vec::new();
        walk(&pkg_dir, "", &live_set, &mut stale);
        for path in stale {
            std::fs::remove_file(&path).expect("bless: remove the stale generated file");
            eprintln!("removed stale {}", path.display());
        }
        return;
    }

    // The disk walk: every directory under rut/tur_host carries a
    // mod.d.rut (rut.jsonc beside the root only); the walked set
    // {rel path → bytes} is EXACTLY the renderer's file set.
    let mut disk: std::collections::BTreeMap<String, String> = Default::default();
    fn walk_dir(dir: &Path, rel: &str, disk: &mut std::collections::BTreeMap<String, String>) {
        let mod_d = dir.join("mod.d.rut");
        assert!(
            mod_d.is_file(),
            "{} must carry mod.d.rut (one directory, one decl module)",
            dir.display()
        );
        // the key is the renderer's rel path (`mod.d.rut` at the root,
        // `NAME/mod.d.rut` under it)
        let key = if rel.is_empty() {
            "mod.d.rut".to_string()
        } else {
            format!("{rel}/mod.d.rut")
        };
        disk.insert(
            key,
            std::fs::read_to_string(&mod_d).expect("read mod.d.rut"),
        );
        for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() {
                let child = if rel.is_empty() {
                    name
                } else {
                    format!("{rel}/{name}")
                };
                walk_dir(&entry.path(), &child, disk);
            } else {
                assert!(
                    name == "mod.d.rut" || name == "rut.jsonc",
                    "unexpected file {} under rut/tur_host — the tree carries \
                     only mod.d.rut files + the manifest (a stray generated \
                     file is a bless-mode miss)",
                    entry.path().display()
                );
            }
        }
    }
    walk_dir(&pkg_dir, "", &mut disk);
    let live_map: std::collections::BTreeMap<String, String> = live
        .iter()
        .map(|(rel, text)| (rel.clone(), text.clone()))
        .collect();
    assert_eq!(
        disk.len(),
        live_map.len(),
        "module-count drift between rut/tur_host and the renderer: disk \
         {:?} vs live {:?}",
        disk.keys().collect::<Vec<_>>(),
        live_map.keys().collect::<Vec<_>>()
    );
    for (rel, text) in &disk {
        assert_eq!(
            live_map.get(rel).map(String::as_str),
            Some(text.as_str()),
            "rut/tur_host/{rel} drifted from the Rust rows — regenerate \
             deliberately: TUR_BLESS_TUR_HOST_DECL=1 cargo nextest run -p \
             tur-integration-tests tur_host_decl"
        );
    }

    // The committed tree keeps LOWERING with a row set EXACTLY the live
    // surface (path + name + arity + types + async lane) — through the
    // driver's REAL discovery lane (a directory walk, the same mount the
    // playground's on-disk pkgs ride).
    let session = rut_native::load_path_session(&pkg_dir)
        .expect("the committed tur_host tree loads through the real walk");
    let mounted = session.pkg("tur_host").expect("the tur_host pkg mounts");
    let MountedBody::Host { host_funcs, .. } = &mounted.body else {
        panic!("the loaded tur_host pkg answered a non-host body");
    };
    let (live_rows, live_consts) = tur_host_surface(&probe_exts());
    assert!(
        live_consts.is_empty(),
        "the standard session grew const rows — the surface grammar has no \
         const spelling; give them a manifest consts table"
    );
    assert_eq!(
        host_funcs.len(),
        live_rows.len(),
        "row-count drift between the committed tree and the live surface"
    );
    let key = |r: &rut_driver::HostRow| format!("{}/{}", r.path, r.name);
    let mounted_by_key: std::collections::BTreeMap<String, &rut_driver::HostRow> =
        host_funcs.iter().map(|r| (key(r), r)).collect();
    for lrow in &live_rows {
        let row = mounted_by_key.get(&key(lrow)).unwrap_or_else(|| {
            panic!(
                "the committed tree is missing the live row `{}` (path `{}`)",
                lrow.name, lrow.path
            )
        });
        assert_eq!(row.path, lrow.path, "module-path drift on `{}`", lrow.name);
        assert_eq!(row.params, lrow.params, "param drift on `{}`", lrow.name);
        assert_eq!(row.ret, lrow.ret, "return drift on `{}`", lrow.name);
        assert_eq!(
            row.is_async, lrow.is_async,
            "async drift on `{}`",
            lrow.name
        );
    }
}

#[test]
fn kit_tree_matches_the_embed() {
    let kit_dir = workspace_root().join("rut/tur_kit");
    let manifest_text = std::fs::read_to_string(kit_dir.join("rut.jsonc")).unwrap();
    let manifest = parse_manifest(&manifest_text).expect("the kit manifest parses");
    assert_eq!(manifest.name.as_deref(), Some("tur_kit"));
    assert!(
        matches!(manifest.pkg_type, PkgType::Lib),
        "the kit is a lib pkg"
    );
    assert!(
        manifest.legacy_entry.lib.is_none() && manifest.legacy_entry.libs.is_empty(),
        "the kit manifest spells the REPEALED entry.lib/entry.libs keys — \
         the root module is the walk's mod.rut"
    );

    // The disk tree: every directory under rut/tur_kit (the root
    // included) carries exactly one mod.rut; no flat `<name>.rut` files
    // remain; rut.jsonc is the only non-mod.rut file allowed.
    let mut disk: std::collections::BTreeMap<String, String> = Default::default();
    fn walk(dir: &Path, rel: &str, disk: &mut std::collections::BTreeMap<String, String>) {
        let mod_rut = dir.join("mod.rut");
        assert!(
            mod_rut.is_file(),
            "{} must carry mod.rut (one directory, one module)",
            dir.display()
        );
        disk.insert(
            rel.to_string(),
            std::fs::read_to_string(&mod_rut).expect("read mod.rut"),
        );
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() {
                let child = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                walk(&entry.path(), &child, disk);
            } else {
                assert!(
                    name == "mod.rut" || name == "rut.jsonc",
                    "unexpected file {} under rut/tur_kit — the tree carries \
                     only mod.rut files + the manifest (a flat <name>.rut is \
                     a loud loader error)",
                    entry.path().display()
                );
            }
        }
    }
    walk(&kit_dir, "", &mut disk);

    // The embed: {"" → TUR_KIT_ROOT} ∪ {mod path → text}. The compiler
    // already errors loudly on decl/row mismatch — the byte-diff is the
    // pin.
    let embedded: std::collections::BTreeMap<String, String> = tur_engine::kit::tur_kit_mods()
        .into_iter()
        .map(|(path, _vis, text)| (path.to_string(), text.to_string()))
        .collect();
    assert_eq!(
        disk.len(),
        embedded.len(),
        "module-count drift between rut/tur_kit and the embed: disk {:?} vs embed {:?}",
        disk.keys().collect::<Vec<_>>(),
        embedded.keys().collect::<Vec<_>>()
    );
    for (path, text) in &disk {
        assert_eq!(
            embedded.get(path).map(String::as_str),
            Some(text.as_str()),
            "rut/tur_kit module `{path}` drifted from the engine's embed"
        );
    }
}

#[test]
fn tur_host_manifest_declares_the_tree() {
    let pkg_dir = workspace_root().join("rut/tur_host");
    let manifest_text = std::fs::read_to_string(pkg_dir.join("rut.jsonc")).unwrap();
    let manifest = parse_manifest(&manifest_text).expect("the tur_host manifest parses");
    assert_eq!(manifest.name.as_deref(), Some("tur_host"));
    assert!(
        matches!(manifest.pkg_type, PkgType::Host),
        "tur_host declares type = \"host\" (the kind is never inferred)"
    );
    assert!(
        manifest.entry.type_path.is_none(),
        "the tur_host manifest spells NO entry — `mod.d.rut` beside the \
         manifest is the auto-discovered default root (lib parity)"
    );
    assert!(
        pkg_dir.join("mod.d.rut").is_file(),
        "the default root surface exists"
    );
}
