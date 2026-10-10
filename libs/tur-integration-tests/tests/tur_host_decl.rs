//! The `tur_host` decl-surface pin + the kit tree pin.
//!
//! Three artifacts are pinned here against the LIVE engine session:
//!
//! 1. `rut/tur_host/tur_host.d.rut` — the GENERATED snapshot of the
//!    standard session's merged `tur_host` surface (`tur_decl_pkg()` +
//!    every plugin's RutPkgExt rows). The Rust rows stay the compile
//!    truth; this test rebuilds the text and diffs it — drift fails the
//!    suite until the snapshot is deliberately regenerated (bless mode:
//!    `TUR_BLESS_TUR_HOST_DECL=1`). The snapshot must also keep lowering
//!    as a decl module whose row set is EXACTLY the live one.
//! 2. `rut/tur_kit/rut.jsonc` — the kit manifest parses with NO repealed
//!    entry keys (the root module is the walk's `mod.rut`), and the
//!    kit's FILE-MODULE TREE on disk (every directory's `mod.rut`) is
//!    byte-identical to the engine's embedded `Pkg.mods` tree.
//! 3. `rut/tur_host/rut.jsonc` — the host pkg's manifest must declare
//!    `type = "host"` and the snapshot as its `entry.type`.

use std::path::{Path, PathBuf};

use rut_driver::bundle::{PkgType, parse_manifest};
use rut_driver::{PkgBody, lower_decl_module};
use tur_animation::TurAnimationPlugin;
use tur_engine::core::capability::Capabilities;
use tur_engine::core::plugin::Plugin;
use tur_engine::core::rut_runtime::{render_tur_host_decl, tur_host_surface};
use tur_engine::core::runtime::probe_register_rut_pkg_exts;
use tur_engine::{TurClipboardPlugin, TurStdPlugin};
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
/// + net (the Http capability present, so the net rows install).
fn standard_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(TurStdPlugin),
        Box::new(TurAnimationPlugin),
        Box::new(TurClipboardPlugin),
        Box::new(TurNetPlugin),
    ]
}

/// The standard session's capabilities: the recording clipboard + a fresh
/// recording http (no requests are ever made — the backends only need to
/// satisfy `TurNetPlugin`'s presence check).
fn standard_capabilities() -> Capabilities {
    let caps = Capabilities::new();
    caps.insert(tur_engine::Clipboard::new(RecordingClipboard::new()));
    caps.insert(tur_net_capability::Http::new(RecordingHttp::new()));
    caps
}

fn live_surface_text() -> String {
    let exts = probe_register_rut_pkg_exts(
        &standard_plugins(),
        standard_capabilities(),
        (400.0, 600.0),
    )
    .expect("the standard session's register probe");
    assert!(
        exts.len() >= 10,
        "the standard session should push every family's ext, got {}",
        exts.len()
    );
    render_tur_host_decl(&exts)
}

#[test]
fn tur_host_decl_snapshot_matches_the_standard_session() {
    let live = live_surface_text();
    let snapshot = workspace_root().join("rut/tur_host/tur_host.d.rut");
    if std::env::var("TUR_BLESS_TUR_HOST_DECL").is_ok() {
        std::fs::write(&snapshot, &live).expect("bless: write the snapshot");
        eprintln!("blessed {}", snapshot.display());
        return;
    }
    let committed = std::fs::read_to_string(&snapshot)
        .expect("rut/tur_host/tur_host.d.rut is committed (see the test header)");
    assert!(
        committed == live,
        "the committed tur_host.d.rut drifted from the Rust rows — regenerate \
         deliberately: TUR_BLESS_TUR_HOST_DECL=1 cargo nextest run -p \
         tur-integration-tests tur_host_decl"
    );

    // The snapshot keeps lowering as a decl module whose row set is
    // EXACTLY the live surface (name + arity + types + async lane).
    let lowered = lower_decl_module(&committed, "tur_host.d.rut")
        .expect("the committed snapshot parses as a decl surface");
    let PkgBody::Host { host_funcs, .. } = lowered.body else {
        panic!("lower_decl_module answered a non-host pkg");
    };
    let (live_rows, live_consts) =
        tur_host_surface(&probe_exts());
    assert!(
        live_consts.is_empty(),
        "the standard session grew const rows — the surface grammar has no \
         const spelling; give them a manifest consts table"
    );
    assert_eq!(
        host_funcs.len(),
        live_rows.len(),
        "row-count drift between the snapshot and the live surface"
    );
    for ((name, ptys, ret, is_async), (lname, lptys, lret, lis_async)) in
        host_funcs.iter().zip(live_rows.iter())
    {
        assert_eq!(name, lname, "row order/name drift");
        assert_eq!(ptys, lptys, "param drift on `{name}`");
        assert_eq!(ret, lret, "return drift on `{name}`");
        assert_eq!(is_async, lis_async, "async drift on `{name}`");
    }
}

fn probe_exts() -> Vec<tur_engine::core::rut_runtime::RutPkgExt> {
    probe_register_rut_pkg_exts(
        &standard_plugins(),
        standard_capabilities(),
        (400.0, 600.0),
    )
    .expect("the standard session's register probe")
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
fn tur_host_manifest_declares_the_snapshot() {
    let manifest_text =
        std::fs::read_to_string(workspace_root().join("rut/tur_host/rut.jsonc")).unwrap();
    let manifest = parse_manifest(&manifest_text).expect("the tur_host manifest parses");
    assert_eq!(manifest.name.as_deref(), Some("tur_host"));
    assert!(
        matches!(manifest.pkg_type, PkgType::Host),
        "tur_host declares type = \"host\" (the kind is never inferred)"
    );
    assert_eq!(
        manifest.entry.type_path.as_deref(),
        Some("./tur_host.d.rut"),
        "the decl surface IS the generated, test-pinned snapshot"
    );
    assert!(
        workspace_root()
            .join("rut/tur_host/tur_host.d.rut")
            .is_file(),
        "the declared snapshot exists"
    );
}
