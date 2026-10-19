//! The precompiled standard kit — generator + pin.
//!
//! The standard offered set (`RutRuntime::offered_pkgs` over THIS
//! harness's standard plugin shape: std + animation + clipboard) is
//! staged to disk and packed through rut's bundle lane into
//! `assets/std_kit.rutbundle` (deterministic: same offered set ⇒
//! byte-identical bundle). `assets/std_kit.fingerprint` records the
//! offered-set fingerprint the bundle was generated from. The harness
//! installs the pair via `set_precompiled_std`; every load whose
//! offered set matches serves the compiled pkgs and compiles only the
//! app module (a different configuration falls back to the source
//! lane — the fingerprint never matches).
//!
//! This test re-packs and diffs both artifacts: drift fails the suite
//! until the bundle is deliberately regenerated (bless mode:
//! `TUR_BLESS_KIT_BUNDLE=1 cargo nextest run -p tur-integration-tests
//! kit_bundle`), which is also the rut-pin bump drill — a new rut
//! compiler changes the packed bytes, never the semantics silently.

use std::fs;
use std::path::{Path, PathBuf};

use rut_driver::{Mounted, MountedBody};
use tur_animation::TurAnimationPlugin;
use tur_engine::core::capability::Capabilities;
use tur_engine::core::plugin::Plugin;
use tur_engine::core::runtime::probe_register_rut_pkg_exts;
use tur_engine::core::rut_runtime::RutRuntime;
use tur_engine::{TurClipboardPlugin, TurStdPlugin};
use tur_integration_tests::RecordingClipboard;

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// The harness's standard (no-http) plugin shape — the configuration
/// `TurTestApp::new` builds, which is the shape the bulk of the corpus
/// runs under and the one the checked-in bundle serves.
fn standard_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(TurStdPlugin),
        Box::new(TurAnimationPlugin),
        Box::new(TurClipboardPlugin),
    ]
}

fn standard_capabilities() -> Capabilities {
    let caps = Capabilities::new();
    caps.insert(tur_engine::Clipboard::new(RecordingClipboard::new()));
    caps
}

/// Stage one offered pkg to a sibling directory (relative `../name` dep
/// paths keep resolving). Manifests derive from the pkg's own manifest
/// (name, kind, deps); the flat host pkg (async_host — root surface
/// only, no mods) spells `entry.type` because the PACK's flat-host arm
/// requires it (the walk's auto-discovery does not cross the pack
/// boundary).
fn stage_pkg(root: &Path, pkg: &Mounted, extra_root_file: Option<(&str, &str)>) {
    let dir = root.join(&pkg.spec);
    fs::create_dir_all(&dir).expect("stage dir");
    let mut manifest = String::new();
    match &pkg.body {
        MountedBody::Host { .. } => {
            manifest.push_str(&format!(
                "{{ \"name\": \"{}\", \"type\": \"host\"",
                pkg.spec
            ));
            if pkg.mods.is_empty() {
                // flat host surface — the pack wants the entry spelled
                manifest.push_str(", \"entry\": { \"type\": \"./mod.d.rut\" }");
            }
        }
        MountedBody::Source { .. } => {
            manifest.push_str(&format!("{{ \"name\": \"{}\"", pkg.spec));
        }
        other => panic!("unexpected body kind for {}: {:?}", pkg.spec, other),
    }
    if !pkg.manifest.deps.is_empty() {
        manifest.push_str(", \"deps\": {");
        let mut first = true;
        for dep in pkg.manifest.deps.keys() {
            if !first {
                manifest.push_str(", ");
            }
            first = false;
            manifest.push_str(&format!("\"{dep}\": {{ \"path\": \"../{dep}\" }}"));
        }
        manifest.push_str(" }");
    }
    manifest.push_str(" }");
    fs::write(dir.join("rut.jsonc"), manifest).expect("manifest");
    match &pkg.body {
        MountedBody::Source { text, .. } => {
            fs::write(dir.join("mod.rut"), text).expect("root module");
        }
        MountedBody::Host { root_surface, .. } => {
            let text = root_surface.as_deref().unwrap_or("");
            fs::write(dir.join("mod.d.rut"), text).expect("root surface");
        }
        _ => unreachable!(),
    }
    for (path, m) in &pkg.mods {
        let mdir = dir.join(path);
        fs::create_dir_all(&mdir).expect("mod dir");
        fs::write(mdir.join(&m.file), &m.text).expect("mod file");
    }
    if let Some((name, text)) = extra_root_file {
        fs::write(dir.join(name), text).expect("extra root file");
    }
}

/// Stage the whole offered set + the synthetic pack root, pack it, and
/// answer (bundle bytes, offered-set fingerprint).
fn generate() -> (Vec<u8>, u64) {
    let exts =
        probe_register_rut_pkg_exts(&standard_plugins(), standard_capabilities(), (400.0, 600.0))
            .expect("the standard session's register probe");
    let offered = RutRuntime::offered_pkgs(&exts);
    let fp = RutRuntime::offered_fingerprint(&offered);

    let tmp = std::env::temp_dir().join(format!("tur-kit-bundle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    for pkg in &offered {
        // async_host lowers FLAT (rows only — no root surface text
        // survives the lowering); stage the vendored decl file verbatim.
        let extra: Option<(&str, &str)> = if pkg.spec == "async_host" {
            let vendored = fs::read_to_string(
                workspace_root().join("libs/tur-engine/src/core/rut_runtime/async_host.d.rut"),
            )
            .expect("the vendored async_host decl");
            Some(("mod.d.rut", Box::leak(vendored.into_boxed_str())))
        } else {
            None
        };
        stage_pkg(&tmp, pkg, extra);
    }

    // The synthetic root: a lib whose closure is the whole offered set —
    // one `use` per dep, so the pack's compile closure LINKS every
    // offered pkg. (`rides_compiled` = linked-by-the-closure: a group
    // the root never uses rides as a source file set, and the load
    // lane would recompile it — the whole win gone.)
    let root = tmp.join("tur_std");
    fs::create_dir_all(&root).expect("root dir");
    fs::write(
        root.join("mod.rut"),
        "// staged root — the offered set's pack root\n\
         use tur_kit::handles::{ mount };\n\
         use tur_anim_kit::{ anim_ctrl };\n\
         use futures::{ sleep };\n",
    )
    .expect("root module");
    let mut deps = String::new();
    for pkg in &offered {
        if pkg.spec == "tur_host" || pkg.spec == "async_host" {
            // pulled transitively by the kit/futures — no direct row
            continue;
        }
        if !deps.is_empty() {
            deps.push_str(", ");
        }
        deps.push_str(&format!(
            "\"{}\": {{ \"path\": \"../{}\" }}",
            pkg.spec, pkg.spec
        ));
    }
    fs::write(
        root.join("rut.jsonc"),
        format!(
            "{{ \"name\": \"tur_std\", \"format\": \"rutbundle\", \"format_version\": 12, \"deps\": {{ {deps} }} }}"
        ),
    )
    .expect("root manifest");

    let (bytes, _symtab) =
        rut_native::pack_dir_opts(&root, &Default::default()).expect("pack the offered set");
    let _ = fs::remove_dir_all(&tmp);
    (bytes, fp)
}

#[test]
fn std_kit_bundle_is_pinned() {
    let (bytes, fp) = generate();
    let bundle_path = assets_dir().join("std_kit.rutbundle");
    let fp_path = assets_dir().join("std_kit.fingerprint");
    let fp_hex = format!("{fp:016x}");

    if std::env::var("TUR_BLESS_KIT_BUNDLE").ok().as_deref() == Some("1") {
        fs::create_dir_all(assets_dir()).expect("assets dir");
        fs::write(&bundle_path, &bytes).expect("bless bundle");
        fs::write(&fp_path, &fp_hex).expect("bless fingerprint");
        eprintln!(
            "blessed std_kit.rutbundle ({} bytes, fp {fp_hex})",
            bytes.len()
        );
        return;
    }

    let pinned = fs::read(&bundle_path).unwrap_or_else(|e| {
        panic!(
            "{} unreadable ({e}) — bless with TUR_BLESS_KIT_BUNDLE=1",
            bundle_path.display()
        )
    });
    assert_eq!(
        bytes, pinned,
        "the packed std kit drifted from assets/std_kit.rutbundle — \
         bless with TUR_BLESS_KIT_BUNDLE=1 (the rut pin bump drill)"
    );
    let pinned_fp = fs::read_to_string(&fp_path)
        .unwrap_or_else(|e| panic!("{} unreadable ({e})", fp_path.display()));
    assert_eq!(
        fp_hex,
        pinned_fp.trim(),
        "the offered-set fingerprint drifted"
    );

    // And the pair round-trips: the pinned bytes decode + the engine's
    // matcher accepts them for exactly this fingerprint.
    let bundle = rut_driver::Bundle::parse(&pinned).expect("decode the pinned bundle");
    let loaded = bundle.walk().expect("walk the pinned bundle");
    assert!(loaded.pkgs.iter().any(|p| p.spec.starts_with("tur_kit")));
}
