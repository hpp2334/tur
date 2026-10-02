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
use tur_engine::core::shell::Cursor;
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

// ---- Phase D: syntax highlighting ----------------------------------------------
// The `pg_highlight` / `pg_apply_highlight` rows: the editor controller's
// span tree carries the boa `code.*` palette, applied at case load + after
// a successful Run (never per keystroke — the spans survive edits until the
// next re-highlight).

/// The code palette (packed `0xRRGGBBAA` — the kit color law): the boa
/// `tokens.ts` `code.*` values the pg rows restate.
const CODE_FG_PLAIN: u64 = 0x1F_25_30_FF; // ink.800 — code.fg
const CODE_KEYWORD: u64 = 0x00_6E_58_FF; // teal.700
const CODE_STRING: u64 = 0x3F_7D_3F_FF; // code.string
const CODE_COMMENT: u64 = 0x8A_94_A3_FF; // ink.500

/// The editor controller's spans as `(text, packed color)` pairs — the
/// highlighting probe (the zero-width/empty-span law is asserted too: an
/// empty-content span panics parley layout).
fn editor_spans(app: &TurTestApp) -> Vec<(String, u64)> {
    let id = editor_editable(app);
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| {
                let c = el.controller();
                c.spans()
                    .iter()
                    .map(|s| {
                        let c = s.color().expect("highlighted spans carry colors");
                        (
                            s.text.clone(),
                            (c.r() as u64) << 24
                                | (c.g() as u64) << 16
                                | (c.b() as u64) << 8
                                | (c.a() as u64),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn joined(spans: &[(String, u64)]) -> String {
    spans.iter().map(|(t, _)| t.as_str()).collect()
}

/// A minimal fixture: one editor Input on a realm-minted controller; the
/// `highlight` probe runs the case-load rail (`pg_highlight` +
/// `pg_apply_highlight`) over a baked-in source — a keyword, an f-string
/// with a hole, and a comment.
const HIGHLIGHT_ROWS_MODULE: &str = r#"
use tur::{ mount, pg_apply_highlight, pg_highlight, st_put, st_take, tctrl_new, undo_new };
use tur_kit::{ Column, Input };

let K_CTRL: u64 = 2;

entry fn start() -> u64 {
    let ctrl = tctrl_new();
    st_put(K_CTRL, ctrl);
    let input = Input.builder().controller(ctrl).undo(undo_new()).width_height(400.0, 200.0)
        .multiline(true).query_key("editor").build();
    mount(Column.builder().child(input).build());
    return 0;
}

// The case-load rail over a known small source.
entry fn highlight(_a: u64, _b: f64) {
    let src = "entry fn start() {\n    let s = f\"x {s}\"; // t\n}\n";
    let ctrl = st_take(K_CTRL);
    pg_apply_highlight(ctrl, pg_highlight(src));
    st_put(K_CTRL, ctrl);
}
"#;

#[test]
fn pg_highlight_rows_color_the_controller_spans() {
    let app = TurTestApp::new_with_extra_plugins(
        600.0,
        400.0,
        vec![Box::new(TurRutPlaygroundPlugin)],
    )
    .unwrap();
    app.load_rut_module(HIGHLIGHT_ROWS_MODULE).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let _ = app.rut_start_answer();

    app.call_rut_entry("highlight", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let spans = editor_spans(&app);
    // The runs tile the source exactly — no zero-width span (the parley
    // trap), no lost text.
    assert_eq!(
        joined(&spans),
        "entry fn start() {\n    let s = f\"x {s}\"; // t\n}\n",
        "the colored runs tile the source"
    );
    let has = |text: &str, color: u64| {
        spans
            .iter()
            .any(|(t, c)| t == text && *c == color)
    };
    assert!(has("entry", CODE_KEYWORD), "the keyword: {spans:?}");
    assert!(has("let", CODE_KEYWORD), "the keyword");
    // The f-string: the prologue + tail color as the string, the hole's
    // identifier overlays plain.
    assert!(has("f\"x {", CODE_STRING), "the f-string prologue");
    assert!(has("}\"", CODE_STRING), "the f-string tail");
    assert!(has("s", CODE_FG_PLAIN), "the hole identifier stays plain");
    assert!(has("// t", CODE_COMMENT), "the comment");
}

#[test]
fn playground_highlights_on_load_and_the_spans_survive_editing() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));

    // The case-load rail highlighted the editor: keyword, string and
    // comment runs ride the controller's span tree.
    let spans = editor_spans(&app);
    let source = joined(&spans);
    assert_eq!(source, case_source("counter"), "the runs tile the case source");
    assert!(
        spans.iter().any(|(t, c)| t == "entry" && *c == CODE_KEYWORD),
        "the keyword run: {spans:?}"
    );
    assert!(spans.iter().any(|(t, c)| t == "let" && *c == CODE_KEYWORD));
    assert!(
        spans.iter().any(|(t, c)| t == "\"Count: 0\"" && *c == CODE_STRING),
        "the string run"
    );
    assert!(spans.iter().any(|(_, c)| *c == CODE_COMMENT), "a comment run");

    // Auto-run off: the only runs below are the explicit ones.
    click_qk(&mut app, &["autorun"]);
    app.wait_for_timeout(Duration::ZERO);

    // A keystroke does NOT destroy the highlighting — the typed char lands
    // in the caret's span and the colored tree survives until the next
    // re-highlight (no per-keystroke apply, no collapse to one plain run).
    focus_editor(&mut app);
    app.send_key("x");
    app.wait_for_timeout(Duration::ZERO);
    let edited = editor_spans(&app);
    assert_eq!(joined(&edited).len(), source.len() + 1, "the keystroke landed");
    assert!(
        edited.iter().any(|(_, c)| *c == CODE_KEYWORD),
        "the keyword runs survived the keystroke"
    );
    assert!(
        edited.iter().any(|(_, c)| *c == CODE_COMMENT),
        "the comment runs survived the keystroke"
    );
    assert!(edited.len() > 4, "the span tree did not collapse: {edited:?}");

    // Undo the keystroke (Backspace deletes left of the caret).
    app.send_key("Backspace");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(joined(&editor_spans(&app)), source, "the text is back");

    // Run: the successful compile re-highlights the editor — the same
    // colored tree over the original source.
    click_qk(&mut app, &["run"]);
    assert!(wait_for_state(&app, "ready"));
    let rerun = editor_spans(&app);
    assert_eq!(joined(&rerun), source, "the run restored the exact source");
    assert!(rerun.iter().any(|(t, c)| t == "entry" && *c == CODE_KEYWORD));
    assert!(rerun.iter().any(|(_, c)| *c == CODE_COMMENT));
    assert!(rerun.iter().any(|(t, c)| t == "\"Count: 0\"" && *c == CODE_STRING));
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

// ---- Phase E: sidebar polish + the divider -------------------------------------
// The sidebar rows paint themselves from the selection/hover atoms (the
// reserved 3px accent bar + the row background) and the divider drags the
// sidebar width atom (clamped 240–720). Colors are sampled in the browser
// pass; here the rails are pinned behaviorally (cursor, geometry, keys).

#[test]
fn playground_sidebar_rows_hover_with_the_pointer_cursor() {
    let mut app = playground_app();

    // Rows carry query keys (`row/<name>` — the selection paint targets;
    // the qkey rows split on `/`, so the query is a two-segment path).
    let row = app
        .query_element(&["row", "clickable-text"])
        .expect("the first row's query key");
    let (cx, cy) = app
        .get_element_absolute_bounds(ElementNodeId::new(row.as_u64()))
        .unwrap()
        .center();

    // Hovering a row applies the region's pointer cursor (the MouseRegion
    // rail the hover paint rides).
    app.pointer_move(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::Pointer),
        "the row's hover cursor"
    );

    // Leaving resets to the default.
    app.pointer_move(1100.0, 690.0);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::Default),
        "the cursor resets off the rows"
    );
}

#[test]
fn playground_select_paints_the_selected_row() {
    let app = playground_app();

    // Select via the probe: the selection atom moves, the toolbar's case
    // name follows, and the selected row's node is addressable.
    app.call_rut_entry("select", case_index("counter"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(
        app.query_text(&["case-name"]).as_deref(),
        Some("counter"),
        "the selection atom moved"
    );
    assert!(
        app.query_element(&["row", "counter"]).is_some(),
        "the selected row is addressable by its query key"
    );

    // Re-selecting another row keeps every rail consistent (the old row's
    // paint drops — the brush sweep in `case_tap`).
    app.call_rut_entry("select", case_index("column-basic"), 0.0).unwrap();
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(app.query_text(&["case-name"]).as_deref(), Some("column-basic"));
}

#[test]
fn playground_divider_drag_resizes_and_clamps_the_sidebar() {
    let mut app = playground_app();

    // Boot width (the atom's seed): 240 — the clamp floor.
    let w0 = qk_width(&app, &["sidebar"]).expect("the sidebar");
    assert_eq!(w0, 240.0, "the seeded sidebar width");

    // The divider sits right of the sidebar; its strip is 8px wide (and
    // moves with the sidebar — re-locate it before every drag).
    let divider_center = |app: &TurTestApp| -> (f64, f64) {
        let divider = app.query_element(&["divider"]).expect("the divider");
        let b = app
            .get_element_absolute_bounds(ElementNodeId::new(divider.as_u64()))
            .unwrap();
        assert_eq!(b.right - b.left, 8.0, "the grab strip is 8px");
        (b.left + 4.0, (b.top + b.bottom) / 2.0)
    };
    let (dx, dy) = divider_center(&app);
    assert!(
        app.take_current_cursor().is_none(),
        "no cursor before hovering the divider"
    );

    // Hovering the strip shows the col-resize cursor (the MouseRegion).
    app.pointer_move(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::ColResize),
        "the divider's col-resize cursor"
    );

    // Drag right by 100: the sidebar widens, the editor pane shrinks.
    app.pointer_down(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    for step in [40.0, 80.0, 100.0] {
        app.pointer_move(dx + step, dy);
        app.wait_for_timeout(Duration::ZERO);
    }
    app.pointer_up(dx + 100.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    let w1 = qk_width(&app, &["sidebar"]).expect("the sidebar");
    assert!((w1 - 340.0).abs() < 2.0, "the drag moved the width 240→{w1}");

    // Post-release moves over the strip must NOT resize (the drag flag
    // gates `div_move`).
    let (dx, dy) = divider_center(&app);
    app.pointer_move(dx + 40.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    let w2 = qk_width(&app, &["sidebar"]).expect("the sidebar");
    assert!((w2 - w1).abs() < 0.5, "a hover move after release is inert: {w1}→{w2}");

    // Clamp high: a huge drag pins at 720.
    let (dx, dy) = divider_center(&app);
    app.pointer_down(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    for step in [100.0, 300.0, 700.0] {
        app.pointer_move(dx + step, dy);
        app.wait_for_timeout(Duration::ZERO);
    }
    app.pointer_up(dx + 700.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(qk_width(&app, &["sidebar"]), Some(720.0), "the upper clamp");

    // Clamp low: a huge leftward drag pins at 240.
    let (dx, dy) = divider_center(&app);
    app.pointer_down(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    for step in [-300.0, -600.0, -1200.0] {
        app.pointer_move(dx + step, dy);
        app.wait_for_timeout(Duration::ZERO);
    }
    app.pointer_up(dx - 1200.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(qk_width(&app, &["sidebar"]), Some(240.0), "the lower clamp");
}
