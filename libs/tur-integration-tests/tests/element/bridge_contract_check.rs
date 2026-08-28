//! Bridge contract guard: every runtime export registered under a module
//! specifier must have a declaration in the corresponding `@tur-ng/*` ambient
//! types, and every declared runtime export must actually be registered.
//!
//! The Rust side registers exports as string-tuple `FnEntry` tables scattered
//! across bridge files; the TS side is hand-written `.d.ts`. Without this test
//! the two can drift silently in both directions:
//! - a newly registered Rust export is invisible to TS until someone uses it;
//! - a renamed/removed export leaves a stale `.d.ts` entry nothing catches.
//!
//! Runtime truth comes from the engine itself (namespace-import the module in
//! an eval'd module, enumerate its own property names). Declared truth is a
//! line scan of the `declare module "<spec>" { ... }` block for runtime export
//! kinds (`function`/`const`/`class`/`enum`) — `interface`/`type` are
//! type-level only and deliberately excluded.

use std::collections::BTreeSet;
use std::path::Path;

use tur_integration_tests::TurTestApp;

/// Repo-relative (spec -> d.ts) pairs under contract. `tur:animation/native`
/// is deliberately absent (hidden internal module); `tur:test` / `tur:cases`
/// are harness fixtures without shipped types.
const CONTRACTS: &[(&str, &str)] = &[
    ("tur:std", "js/packages/tur-std/src/index.d.ts"),
    ("tur:core", "js/packages/tur-core/src/index.d.ts"),
    ("tur:animation", "js/packages/tur-animation/src/index.d.ts"),
    ("tur:clipboard", "js/packages/tur-clipboard/src/index.d.ts"),
    ("tur:net", "js/packages/tur-net/src/index.d.ts"),
    (
        "tur:filepicker",
        "js/packages/tur-filepicker/src/index.d.ts",
    ),
];

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to resolve workspace root")
        .to_path_buf()
}

/// Extract the runtime-export names declared inside `declare module "<spec>"`.
/// `export * from "X"` re-exports are resolved recursively against the other
/// contract entries (ambient `.d.ts` can't express them inline).
fn declared_runtime_exports(spec: &str, rel_path: &str) -> Vec<String> {
    let path = workspace_root().join(rel_path);
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut names = Vec::new();
    let mut inside = false;
    for line in src.lines() {
        if !inside {
            if line.contains(&format!("declare module \"{spec}\"")) {
                inside = true;
            }
            continue;
        }
        // Single module block per file; a column-0 `}` closes it.
        if line == "}" {
            break;
        }
        let trimmed = line.trim_start();
        // `export * from "<other>"` — union the other module's declarations.
        if let Some(rest) = trimmed.strip_prefix("export * from ")
            && let Some(other) = rest
                .trim_end_matches(';')
                .trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
            && let Some((_, other_path)) = CONTRACTS.iter().find(|(s, _)| *s == other)
        {
            names.extend(declared_runtime_exports(other, other_path));
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("export ") else {
            continue;
        };
        let rest = rest.strip_prefix("declare ").unwrap_or(rest);
        for kind in ["function", "const", "class", "enum"] {
            if let Some(after) = rest.strip_prefix(kind)
                && after.starts_with(char::is_whitespace)
            {
                let name: String = after
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if !name.is_empty() {
                    names.push(name);
                }
                break;
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Enumerate a native module's exports through the engine itself.
fn runtime_exports(app: &TurTestApp, spec: &str) -> Vec<String> {
    app.eval_module_source(&format!(
        r#"
import * as m from "{spec}";
globalThis.__export_names = Object.getOwnPropertyNames(m).sort().join(",");
export function start() {{}}
"#
    ))
    .unwrap_or_else(|e| panic!("enumerate {spec}: {e}"));
    let joined = app.eval_js("globalThis.__export_names");
    joined
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn assert_contract(spec: &str, dts_path: &str, runtime: &[String], declared: &[String]) {
    let r: BTreeSet<&String> = runtime.iter().collect();
    let d: BTreeSet<&String> = declared.iter().collect();
    let untyped: Vec<_> = r.difference(&d).collect();
    let unregistered: Vec<_> = d.difference(&r).collect();
    assert!(
        untyped.is_empty() && unregistered.is_empty(),
        "bridge contract drift for {spec}:\n  registered but untyped:   {untyped:?}\n  typed but unregistered: {unregistered:?}"
    );
}

#[test]
fn tur_std_and_core_and_animation_and_clipboard_match_their_types() {
    let app = TurTestApp::new(100.0, 100.0).unwrap(); // Std + Animation + Clipboard plugins
    for (spec, dts) in [
        ("tur:std", CONTRACTS[0].1),
        ("tur:core", CONTRACTS[1].1),
        ("tur:animation", CONTRACTS[2].1),
        ("tur:clipboard", CONTRACTS[3].1),
    ] {
        let runtime = runtime_exports(&app, spec);
        assert!(!runtime.is_empty(), "{spec} should register exports");
        assert_contract(spec, dts, &runtime, &declared_runtime_exports(spec, dts));
    }
}

#[test]
fn tur_net_matches_its_types() {
    // `tur:net` registers only when an Http backend exists.
    let app = TurTestApp::new_with_http(100.0, 100.0).unwrap();
    let runtime = runtime_exports(&app, "tur:net");
    assert!(!runtime.is_empty(), "tur:net should register exports");
    assert_contract(
        "tur:net",
        CONTRACTS[4].1,
        &runtime,
        &declared_runtime_exports("tur:net", CONTRACTS[4].1),
    );
}

#[test]
fn tur_filepicker_matches_its_types() {
    let app = TurTestApp::new_with_filepicker(100.0, 100.0).unwrap();
    let runtime = runtime_exports(&app, "tur:filepicker");
    assert!(
        !runtime.is_empty(),
        "tur:filepicker should register exports"
    );
    assert_contract(
        "tur:filepicker",
        CONTRACTS[5].1,
        &runtime,
        &declared_runtime_exports("tur:filepicker", CONTRACTS[5].1),
    );
}
