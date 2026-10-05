//! The corpus compile gate: every rut case in
//! `js/packages/tur-test-cases/cases` must compile against the standard
//! plugin set (the callback rail's corpus-wide pin — a case that
//! regressions to a stringly-typed callback or a stale row name fails
//! here, per case, with its own load error).

use std::path::Path;
use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn every_corpus_case_compiles_against_the_standard_kit() {
    let cases_dir = workspace_root().join("js/packages/tur-test-cases/cases");
    let mut names: Vec<String> = std::fs::read_dir(&cases_dir)
        .expect("cases dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| cases_dir.join(n).join("index.rut").exists())
        .collect();
    names.sort();

    assert!(
        names.len() >= 100,
        "the corpus should sweep every case, found {}",
        names.len()
    );

    let mut failures = Vec::new();
    for name in &names {
        let source =
            std::fs::read_to_string(cases_dir.join(name).join("index.rut")).unwrap();
        // The browser-shaped capability set: showcase cases ride the net
        // rows (github-viewer), which only install when Http is present.
        let app = TurTestApp::new_with_http(400.0, 600.0).unwrap();
        if let Err(e) = app.load_rut_module(&source) {
            failures.push(format!("{name}: {e}"));
        } else {
            app.wait_for_timeout(Duration::ZERO);
        }
    }
    assert!(
        failures.is_empty(),
        "{} corpus case(s) failed to compile:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
