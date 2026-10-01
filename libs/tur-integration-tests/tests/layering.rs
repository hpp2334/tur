//! The layering law — the phase-6 gate, pinned as a source-scan test.
//!
//! `core/` is MECHANISM ONLY: it must never reference `builtin_plugins`
//! (element/view types live in the plugins), and the shared builder
//! contract must never reappear (no `RutBuilder` trait, no generic
//! `el_build` / `el_child` / `el_qkey` / `el_vqkey` rows outside the
//! plugins — every family owns its spec + rows + its own `*_build`).

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn core_is_mechanism_only() {
    let core = workspace_root().join("libs/tur-engine/src/core");
    let mut files = Vec::new();
    collect_rs(&core, &mut files);
    assert!(
        files.len() > 50,
        "core source scan found {} files — the walker is broken",
        files.len()
    );

    let mut violations = Vec::new();
    for file in files {
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (line_no, line) in src.lines().enumerate() {
            if line.contains("builtin_plugins") {
                violations.push(format!(
                    "{}:{}: core references builtin_plugins: {}",
                    file.display(),
                    line_no + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "the layering law: core must own mechanism, never elements —\n{}",
        violations.join("\n")
    );
}

#[test]
fn no_shared_builder_contract() {
    // The shared builder contract is gone: every element family owns its
    // spec + rows + its own `*_build` terminal. The generic rows and the
    // `RutBuilder`-style trait must never reappear outside the plugins
    // (the kit wraps family rows; core knows nothing of builders).
    let engine = workspace_root().join("libs/tur-engine/src");
    let mut files = Vec::new();
    collect_rs(&engine, &mut files);

    let forbidden = ["el_build(", "el_child(", "el_qkey(", "el_vqkey("];
    let mut violations = Vec::new();
    for file in files {
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (line_no, line) in src.lines().enumerate() {
            // `RutBuilder` anywhere is a violation.
            if line.contains("RutBuilder") {
                violations.push(format!(
                    "{}:{}: a shared RutBuilder contract reappeared: {}",
                    file.display(),
                    line_no + 1,
                    line.trim()
                ));
            }
            for row in forbidden {
                if line.contains(row) {
                    violations.push(format!(
                        "{}:{}: the generic `{}` row reappeared: {}",
                        file.display(),
                        line_no + 1,
                        row.trim_end_matches('('),
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "the layering law: every family owns its rows; no shared builder \
         contract may reappear —\n{}",
        violations.join("\n")
    );
}
