//! Phase B — the playground viewer gate: the playground module
//! (`playground.rut` + the generated `cases_gen.rut`, the same
//! concatenation the website's `playgroundSource()` loads) runs a case in
//! a hosted child instance through a bound `VirtualAppView`.
//!
//! The drive mirrors the browser: a sidebar tap (the real intent path)
//! and the probe entries (`select` / `probe` — the corpus convention)
//! exercise `run_case`'s destroy-then-spawn controller swap, and the
//! status rail must settle on `ok` (the poller's `va_status` read) with
//! the child live under the viewer.

use std::time::Duration;

use tur_integration_tests::TurTestApp;
use tur_playground::TurRutPlaygroundPlugin;

const PLAYGROUND_RUT: &str = include_str!("../../../demo/playground-view/playground.rut");
const CASES_GEN_RUT: &str = include_str!("../../../demo/playground-view/cases_gen.rut");

/// The website's concatenation (`playgroundSource()`): one loadable module.
fn playground_source() -> String {
    format!("{PLAYGROUND_RUT}\n{CASES_GEN_RUT}")
}

/// The sidebar index of a corpus case (the gen-cases ordering: sorted
/// readdir of `js/packages/tur-test-cases/cases`, `index.rut` dirs only).
fn case_index(name: &str) -> u64 {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let dir = root.join("js/packages/tur-test-cases/cases");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && e.path().join("index.rut").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
        .iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("case `{name}` not in the corpus"))
        as u64
}

fn playground_app() -> TurTestApp {
    let app = TurTestApp::new_with_extra_plugins(
        1200.0,
        700.0,
        vec![Box::new(TurRutPlaygroundPlugin)],
    )
    .unwrap();
    app.load_rut_module(&playground_source()).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app
}

/// Drive `select` and poll the status line until it reads `want` (the
/// poller settles on the real clock — the child compiles the kit prelude
/// on the virtual-pool worker).
fn wait_for_status(app: &TurTestApp, want: &str) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        app.wait_for_timeout(Duration::from_millis(64));
        if app.query_text(&["status"]).as_deref() == Some(want) {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
    }
}

#[test]
fn playground_boots_and_shows_the_no_case_fallback() {
    let app = playground_app();

    // The status line's atom rides the start answer (the probe channel).
    let _status_atom = app.rut_start_answer();
    assert_eq!(
        app.query_text(&["status"]).as_deref(),
        Some("pick a case"),
        "the initial status line"
    );
    // The viewer is mounted (the always-live fallback branch) with the
    // FallbackView shape showing.
    assert_eq!(
        app.query_text(&["viewer", "hint"]).as_deref(),
        Some("(no case)"),
        "the FallbackView hint before any case runs"
    );
    let id = app.query_element(&["viewer"]).expect("the viewer host node");
    let node = app
        .dev_tool_get_element(id)
        .expect("the viewer host in the dev-tool tree");
    assert_eq!(node.name, "tur_virtual_app", "the viewer hosts a VirtualAppView");
    assert!(
        node.size.0 > 0.0 && node.size.1 > 0.0,
        "the viewer pane laid out: {:?}",
        node.size
    );
}

#[test]
fn playground_viewer_runs_counter_to_ready() {
    let app = playground_app();
    let status_atom = app.rut_start_answer();

    // Run the `counter` case through the probe entry.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(
        wait_for_status(&app, "ok"),
        "the child never reached ready: {:?}",
        app.query_text(&["status"])
    );

    // The explicit `va_status` probe reads the live controller's rail.
    app.call_rut_entry("probe", status_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["status"]).as_deref(),
        Some("running"),
        "the probe entry's va_status read"
    );

    // The child settled: the hint overlay dropped and the host element is
    // live in the tree (the child's frame replays through its paint).
    assert!(
        app.query_text(&["viewer", "hint"])
            .is_none_or(|h| h.is_empty()),
        "the FallbackView hint was dropped once ready"
    );
    let id = app.query_element(&["viewer"]).expect("the viewer host node");
    let node = app.dev_tool_get_element(id).unwrap();
    assert_eq!(node.name, "tur_virtual_app");
    assert!(
        node.size.0 > 0.0 && node.size.1 > 0.0,
        "the hosting element kept its layout: {:?}",
        node.size
    );
}

#[test]
fn playground_run_swaps_controllers_destroy_then_spawn() {
    let mut app = playground_app();

    // First run via the REAL intent path: a sidebar tap on the first row
    // (clickable-text — the corpus's sorted head).
    app.click(110.0, 16.0);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        wait_for_status(&app, "ok"),
        "the sidebar tap never ran the case: {:?}",
        app.query_text(&["status"])
    );

    // Second run via the probe: the swap destroys the first child and
    // spawns a fresh one under a new controller; the new child must reach
    // `running` on its own rail.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(
        wait_for_status(&app, "ok"),
        "the swapped-in child never reached ready: {:?}",
        app.query_text(&["status"])
    );
    let status_atom = app.rut_start_answer();
    app.call_rut_entry("probe", status_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["status"]).as_deref(),
        Some("running"),
        "the swapped-in controller's va_status"
    );
}

#[test]
fn playground_run_button_reruns_the_case() {
    let mut app = playground_app();

    // First run via the probe entry.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_status(&app, "ok"));

    // The Run button (toolbar row above the editor: pane x 220..710,
    // header 44 + toolbar 40 with 6px padding, 72x28 pill) re-runs the
    // editor's current source through the real intent queue.
    app.click(262.0, 64.0);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        wait_for_status(&app, "ok"),
        "the Run button never respawned the child: {:?}",
        app.query_text(&["status"])
    );
    let status_atom = app.rut_start_answer();
    app.call_rut_entry("probe", status_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["status"]).as_deref(),
        Some("running"),
        "a re-run respawns a fresh child"
    );
}
