//! The playground gate (Phase B rail + Phase C toolbar): the playground
//! module (`playground.rut` + the generated `cases_gen.rut`, the same
//! concatenation the website's `playgroundSource()` loads) runs a case in
//! a hosted child instance through a bound `VirtualAppView`.
//!
//! Phase B: a sidebar tap (the real intent path) and the probe entries
//! (`select` / `probe` — the corpus convention) exercise `run_case`'s
//! destroy-then-spawn controller swap, and the status rail must settle
//! with the child live under the viewer.
//!
//! Phase C: the toolbar (Run / Reset / auto-run / Split-Edit-View tabs),
//! the status bar (state dot + label, edited pill, compiled-ago) and the
//! `input_on_input` edit rail (the debounced auto-run).

use std::time::Duration;

use tur_engine::builtin_plugins::text::elements::EditableTextElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
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
    let dir = cases_dir();
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

/// A corpus case's original source (what Reset restores).
fn case_source(name: &str) -> String {
    std::fs::read_to_string(cases_dir().join(name).join("index.rut"))
        .unwrap_or_else(|e| panic!("case `{name}` source: {e}"))
}

fn cases_dir() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    root.join("js/packages/tur-test-cases/cases")
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

/// The `qk`-tagged element's absolute center (click helpers drive real
/// hit tests — no hardcoded geometry).
fn qk_center(app: &TurTestApp, qk: &[&str]) -> (f64, f64) {
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let id = ElementNodeId::new(id.as_u64());
    app.get_element_absolute_bounds(id).unwrap().center()
}

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    let (cx, cy) = qk_center(app, qk);
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
}

/// The `qk`-tagged element's laid-out width (the pane-switch probes).
fn qk_width(app: &TurTestApp, qk: &[&str]) -> Option<f64> {
    let id = app.query_element(qk)?;
    let b = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap();
    Some(b.right - b.left)
}

/// Locate the `EditableTextElement` under the editor's query key (Input
/// puts the key on its Container wrapper).
fn editor_editable(app: &TurTestApp) -> ElementNodeId {
    let container_id = app.query_element(&["editor"]).expect("editor not found");
    let container_id = ElementNodeId::new(container_id.as_u64());
    let tree = app.element_tree();
    let mut stack: Vec<ElementNodeId> = tree
        .get_element(container_id)
        .unwrap()
        .children
        .iter()
        .map(|c| ElementNodeId::new(c.as_u64()))
        .collect();
    while let Some(id) = stack.pop() {
        let node = tree.get_element(id).unwrap();
        if node.kind() == Some(ElementKind::new("tur_editable_text")) {
            return id;
        }
        for c in &node.children {
            stack.push(ElementNodeId::new(c.as_u64()));
        }
    }
    panic!("no tur_editable_text under the editor");
}

fn editor_text(app: &TurTestApp) -> String {
    let id = editor_editable(app);
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn focus_editor(app: &mut TurTestApp) {
    let id = editor_editable(app);
    // Click inside the VISIBLE editor region (the pane's left half) — the
    // editable's own bounds can reach under the viewer pane in split
    // mode, where the click would hit the viewer instead.
    let b = app.get_element_absolute_bounds(id).unwrap();
    let (cx, cy) = (b.left + 60.0, (b.top + b.bottom) / 2.0);
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
}

/// Drive `select` and poll the app-state label until it reads `want` (the
/// poller settles on the real clock — the child compiles the kit prelude
/// on the virtual-pool worker).
fn wait_for_state(app: &TurTestApp, want: &str) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        app.wait_for_timeout(Duration::from_millis(64));
        if app.query_text(&["app-state"]).as_deref() == Some(want) {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
    }
}

// ---- Phase B rail -----------------------------------------------------------

#[test]
fn playground_boots_and_shows_the_no_case_fallback() {
    let app = playground_app();

    // The state atom rides the start answer (the probe channel); the
    // status bar's label reads `ready` from boot.
    let _state_atom = app.rut_start_answer();
    assert_eq!(
        app.query_text(&["app-state"]).as_deref(),
        Some("ready"),
        "the initial app-state label"
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
    let state_atom = app.rut_start_answer();

    // Run the `counter` case through the probe entry.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(
        wait_for_state(&app, "ready"),
        "the child never reached ready: {:?}",
        app.query_text(&["app-state"])
    );

    // The explicit `va_status` probe writes the live controller's rail
    // into the state atom (the start answer) — the label flips to the raw
    // `running` read.
    app.call_rut_entry("probe", state_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["app-state"]).as_deref(),
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
    app.click(110.0, 56.0);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        wait_for_state(&app, "ready"),
        "the sidebar tap never ran the case: {:?}",
        app.query_text(&["app-state"])
    );

    // Second run via the probe: the swap destroys the first child and
    // spawns a fresh one under a new controller; the new child must reach
    // ready on its own rail.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(
        wait_for_state(&app, "ready"),
        "the swapped-in child never reached ready: {:?}",
        app.query_text(&["app-state"])
    );
}

#[test]
fn playground_run_button_reruns_the_case() {
    let mut app = playground_app();

    // First run via the probe entry.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));

    // The Run button (the toolbar's right action cluster) re-runs the
    // editor's current source through the real intent queue.
    click_qk(&mut app, &["run"]);
    assert!(
        wait_for_state(&app, "ready"),
        "the Run button never respawned the child: {:?}",
        app.query_text(&["app-state"])
    );
}

// ---- Phase C: layout tabs ----------------------------------------------------

#[test]
fn playground_layout_tabs_switch_panes() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));

    // Split (the boot mode): both panes share the row.
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert!(editor_w > 300.0 && viewer_w > 300.0, "split panes: {editor_w} / {viewer_w}");

    // Edit: the viewer shells into a zero-width, hard-clipped container
    // (it stays mounted — presence unchanged, width 0) and the editor
    // takes the whole row.
    click_qk(&mut app, &["tab-edit"]);
    app.wait_for_timeout(Duration::ZERO);
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert!(editor_w > 900.0, "the editor fills the row in edit mode: {editor_w}");
    assert_eq!(viewer_w, 0.0, "the viewer pane collapsed in edit mode");
    // The child survived the layout churn (the viewer kept its host).
    assert_eq!(
        app.dev_tool_get_element(app.query_element(&["viewer"]).unwrap())
            .unwrap()
            .name,
        "tur_virtual_app"
    );

    // View: the editor collapses instead.
    click_qk(&mut app, &["tab-view"]);
    app.wait_for_timeout(Duration::ZERO);
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert_eq!(editor_w, 0.0, "the editor pane collapsed in view mode");
    assert!(viewer_w > 900.0, "the viewer fills the row in view mode: {viewer_w}");

    // Back to split.
    click_qk(&mut app, &["tab-split"]);
    app.wait_for_timeout(Duration::ZERO);
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert!(editor_w > 300.0 && viewer_w > 300.0, "split again: {editor_w} / {viewer_w}");
}

// ---- Phase C: reset -----------------------------------------------------------

#[test]
fn playground_reset_restores_the_original_source() {
    let mut app = playground_app();
    let original = case_source("counter");
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(editor_text(&app), original, "the editor loaded the case source");
    assert!(app.query_element(&["edited-pill"]).is_none(), "not edited after a clean load");

    // Type a keystroke: the text diverges (the edited pill appears).
    focus_editor(&mut app);
    app.send_key("x");
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["edited-pill"]).is_some(), "the edited pill after a keystroke");

    // Reset: the original source returns and the case re-runs (the pill
    // clears — the compiled baseline moved back to the original).
    click_qk(&mut app, &["reset"]);
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(editor_text(&app), original, "Reset restored the case source");
    assert!(
        app.query_element(&["edited-pill"]).is_none(),
        "the edited pill cleared after the reset re-ran"
    );
}

// ---- Phase C: auto-run + the edit rail ----------------------------------------

#[test]
fn playground_auto_run_respawns_after_the_debounce() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));

    // Auto-run is ON from boot (the boa default).
    assert_eq!(app.query_text(&["auto-state"]).as_deref(), Some("auto-run on"));

    // A keystroke breaks the source; after the 500 ms debounce the
    // auto-run recompiles and the viewer flips to the compile error.
    focus_editor(&mut app);
    app.send_key("x");
    app.wait_for_timeout(Duration::from_secs(1));
    assert_eq!(
        app.query_text(&["app-state"]).as_deref(),
        Some("compile-error"),
        "the debounced auto-run recompiled the broken source"
    );
    // The compiled-ago clock restarted with the (failed) run attempt —
    // the label is present either way.
    assert!(
        app.query_text(&["compiled-ago"]).is_some_and(|s| s.starts_with("compiled ")),
        "the compiled-ago label: {:?}",
        app.query_text(&["compiled-ago"])
    );
}

#[test]
fn playground_auto_run_off_keeps_the_case_running() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));

    // Toggle auto-run OFF (the toolbar pill).
    click_qk(&mut app, &["autorun"]);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["auto-state"]).as_deref(), Some("auto-run off"));

    // A keystroke still marks the editor edited... but never respawns:
    // the case keeps running well past the debounce window.
    focus_editor(&mut app);
    app.send_key("x");
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["edited-pill"]).is_some(), "the edited pill appears");
    app.wait_for_timeout(Duration::from_secs(1));
    assert_eq!(
        app.query_text(&["app-state"]).as_deref(),
        Some("ready"),
        "auto-run off: no debounced recompile"
    );

    // Toggle back on (the pill flips in the status bar).
    click_qk(&mut app, &["autorun"]);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["auto-state"]).as_deref(), Some("auto-run on"));
}

// ---- Phase C: the `input_on_input` row (native pin) ----------------------------

/// A minimal fixture: one Input on a realm controller wired through the
/// kit's `.on_input(name, id)` (the `input_on_input` row). Every user
/// edit delivers the named entry with the row's id crossing.
const INPUT_ON_INPUT_MODULE: &str = r#"
use tur::{ mount, rs_set_str, rs_source_str, st_put, stf_put, stf_take, tctrl_new, undo_new };
use tur_kit::{ Column, Input, Text };

let K_TEXT: u64 = 1;
let K_CTRL: u64 = 2;

entry fn start() -> u64 {
    let text = rs_source_str("cold");
    stf_put(K_TEXT, text as f64);
    let ctrl = tctrl_new();
    st_put(K_CTRL, ctrl);
    let input = Input.builder().controller(ctrl).undo(undo_new()).width_height(400.0, 200.0)
        .on_input("on_edit", 7).query_key("input").build();
    let mut col = Column.builder();
    col.child(input);
    col.child(Text.builder().text_bound(text).query_key("echo").build());
    mount(col.build());
    return text;
}

// The edit intent: the id crosses from the row argument (7).
entry fn on_edit(id: u64, _b: u64, _n: f64) {
    let text = stf_take(K_TEXT) as u64;
    rs_set_str(text, f"edit:{id}");
    stf_put(K_TEXT, text as f64);
}
"#;

#[test]
fn input_on_input_row_fires_the_edit_intent() {
    let mut app = TurTestApp::new(600.0, 400.0).unwrap();
    app.load_rut_module(INPUT_ON_INPUT_MODULE).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let _text_atom = app.rut_start_answer();
    assert_eq!(app.query_text(&["echo"]).as_deref(), Some("cold"), "the pre-edit echo");

    // Focus the input and type: the intent fires with the row's id.
    let container_id = app.query_element(&["input"]).expect("the input");
    let editable = {
        let tree = app.element_tree();
        let mut stack: Vec<ElementNodeId> = tree
            .get_element(ElementNodeId::new(container_id.as_u64()))
            .unwrap()
            .children
            .iter()
            .map(|c| ElementNodeId::new(c.as_u64()))
            .collect();
        let mut found = None;
        while let Some(id) = stack.pop() {
            let node = tree.get_element(id).unwrap();
            if node.kind() == Some(ElementKind::new("tur_editable_text")) {
                found = Some(id);
                break;
            }
            for c in &node.children {
                stack.push(ElementNodeId::new(c.as_u64()));
            }
        }
        found.expect("the editable under the input")
    };
    let (cx, cy) = app.get_element_absolute_bounds(editable).unwrap().center();
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    app.send_key("x");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["echo"]).as_deref(),
        Some("edit:7"),
        "the on-input intent delivered the row's id"
    );
}
