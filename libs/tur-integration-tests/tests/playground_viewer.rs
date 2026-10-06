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

/// The workspace root (the tests' fixtures live outside the crate).
fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// The curated showcase manifest (`demo/playground-view/showcase.json`) —
/// the same file `gen-cases.cjs` consumes. The registry emits it in
/// alphabetical order (the generator sorts, like the boa reference), which
/// is the sidebar order the `select` probes address. A plain comma split
/// is enough: showcase names are lowercase-hyphen identifiers.
fn showcase_names() -> Vec<String> {
    let raw = std::fs::read_to_string(workspace_root().join("demo/playground-view/showcase.json"))
        .expect("demo/playground-view/showcase.json");
    let raw = raw.trim();
    assert!(
        raw.starts_with('[') && raw.ends_with(']'),
        "showcase.json is a JSON array of names"
    );
    raw[1..raw.len() - 1]
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The sidebar index of a showcase case (the gen-cases ordering: the
/// `showcase.json` manifest, alphabetical — NOT the whole corpus dir).
fn case_index(name: &str) -> u64 {
    showcase_names()
        .iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("case `{name}` not in demo/playground-view/showcase.json"))
        as u64
}

/// A showcase case's original source (what Reset restores).
fn case_source(name: &str) -> String {
    std::fs::read_to_string(cases_dir().join(name).join("index.rut"))
        .unwrap_or_else(|e| panic!("case `{name}` source: {e}"))
}

fn cases_dir() -> std::path::PathBuf {
    workspace_root().join("js/packages/tur-test-cases/cases")
}

fn playground_app() -> TurTestApp {
    // The browser-shaped capability set: the website's runtime registers
    // `Http` (tur-net-wasm), so the playground instance must have it too
    // for the net-riding showcase cases (github-viewer) to compile.
    let app = TurTestApp::new_with_http_and_plugins(
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

// ---- Boot selection (the phase-2 chrome pass) --------------------------------

#[test]
fn playground_boot_auto_selects_counter() {
    let app = playground_app();
    let _state_atom = app.rut_start_answer();

    // The boot selection (the boa INITIAL_CASE): the toolbar's case name
    // reads `counter` before any tap.
    assert_eq!(
        app.query_text(&["case-name"]).as_deref(),
        Some("counter"),
        "the boot selection"
    );

    // The case is live: the status settles to ready, the viewer hosts the
    // child with the FallbackView hint dropped, and the editor loaded the
    // case's source.
    assert!(
        wait_for_state(&app, "ready"),
        "the boot case never reached ready: {:?}",
        app.query_text(&["app-state"])
    );
    assert!(
        app.query_text(&["viewer", "hint"])
            .is_none_or(|h| h.is_empty()),
        "the FallbackView hint dropped once the boot case ran"
    );
    let id = app
        .query_element(&["viewer"])
        .expect("the viewer host node");
    let node = app
        .dev_tool_get_element(id)
        .expect("the viewer host in the dev-tool tree");
    assert_eq!(
        node.name, "tur_virtual_app",
        "the viewer hosts a VirtualAppView"
    );
    assert!(
        node.size.0 > 0.0 && node.size.1 > 0.0,
        "the viewer pane laid out: {:?}",
        node.size
    );
    assert_eq!(
        editor_text(&app),
        case_source("counter"),
        "the editor loaded the boot case's source"
    );
}

// ---- The phase-2 chrome pins: metrics + structure -----------------------------
// The boa chrome metrics (style parity, not pixel-perfect): a 48px toolbar
// with the compact icon buttons + the "auto" caption, a 200px sidebar with
// the CASES header + full-width inset row pills, the ~20px bordered status
// bar, the inset viewer card (no header strip), and the visible 8px
// divider bands.

#[test]
fn playground_chrome_metrics_match_the_boa_reference() {
    let app = playground_app();
    let _ = app.rut_start_answer();

    // Toolbar: 48px band; the status bar: ~20px + hairlines (22 with the
    // inside border); the sidebar: the boa 200 seed.
    let height = |qk: &[&str]| -> f64 {
        let id = app
            .query_element(qk)
            .unwrap_or_else(|| panic!("{qk:?} not found"));
        let b = app
            .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
            .unwrap();
        b.bottom - b.top
    };
    assert_eq!(height(&["toolbar"]), 48.0, "the toolbar band");
    assert_eq!(
        height(&["status-bar"]),
        22.0,
        "the status bar band (20 + inside border)"
    );
    assert_eq!(
        qk_width(&app, &["sidebar"]),
        Some(200.0),
        "the sidebar seed"
    );

    // The toolbar's auto caption + the sidebar's CASES header (the count
    // mirrors the generated registry = the showcase manifest).
    assert_eq!(app.query_text(&["auto-caption"]).as_deref(), Some("auto"));
    assert_eq!(
        app.query_text(&["cases-count"]).as_deref(),
        Some(showcase_names().len().to_string().as_str()),
        "the CASES header count"
    );

    // The status bar's off-state is the "⌘S to run" hint (auto-run is ON
    // at boot, so flip it first).
    let mut app = app;
    click_qk(&mut app, &["autorun"]);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["auto-state"]).as_deref(),
        Some("\u{2318}S to run"),
        "the auto-run off caption"
    );

    // The viewer card: the inset (pane − 24) with no header strip above —
    // the card fills the pane minus the 12px ring on every side.
    let pane_w = qk_width(&app, &["viewer-pane"]).expect("the viewer pane");
    let card_w = qk_width(&app, &["viewer-card"]).expect("the viewer card");
    assert!(
        (pane_w - card_w - 24.0).abs() < 0.5,
        "the viewer card is inset 12px per side: pane {pane_w} vs card {card_w}"
    );
}

#[test]
fn playground_sidebar_rows_are_full_width_left_aligned_pills() {
    let app = playground_app();
    let _ = app.rut_start_answer();

    // The row's qk sits on the padding-8 wrapper: full sidebar width (the
    // full-width inset pill law), natural boa pitch (2 + 8 + pill + 8 ≈
    // 47.5 with the 13px label).
    let id = app
        .query_element(&["row", "counter"])
        .expect("the showcase row's query key");
    let b = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap();
    assert_eq!(b.right - b.left, 200.0, "the row spans the sidebar");
    let h = b.bottom - b.top;
    assert!(
        (44.0..=52.0).contains(&h),
        "the row pitch is the boa ~47.5: {h}"
    );
}

#[test]
fn playground_viewer_runs_counter_to_ready() {
    let app = playground_app();
    let state_atom = app.rut_start_answer();

    // Run the `counter` case through the probe entry.
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
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
    let id = app
        .query_element(&["viewer"])
        .expect("the viewer host node");
    let node = app.dev_tool_get_element(id).unwrap();
    assert_eq!(node.name, "tur_virtual_app");
    assert!(
        node.size.0 > 0.0 && node.size.1 > 0.0,
        "the hosting element kept its layout: {:?}",
        node.size
    );
}

#[test]
fn playground_viewer_compiles_the_net_riding_github_viewer() {
    // github-viewer is the showcase's net row rider: the sidebar select
    // compiles it in the child instance (the net rows exist here because
    // `playground_app` registers the Http capability, matching the
    // website's runtime) and boots it to ready with the viewer hosting a
    // live child. (The child's landing tree is pinned content-side by
    // `event/github_viewer.rs`.)
    let app = playground_app();
    app.call_rut_entry("select", case_index("github-viewer"), 0.0)
        .unwrap();
    assert!(
        wait_for_state(&app, "ready"),
        "the child never reached ready: {:?}",
        app.query_text(&["app-state"])
    );
    let node = app
        .dev_tool_get_element(app.query_element(&["viewer"]).unwrap())
        .expect("the viewer host in the dev-tool tree");
    assert_eq!(node.name, "tur_virtual_app");
    assert!(
        node.size.0 > 0.0 && node.size.1 > 0.0,
        "the hosting element kept its layout: {:?}",
        node.size
    );
}

// ---- Phase B rail -----------------------------------------------------------

#[test]
fn playground_run_swaps_controllers_destroy_then_spawn() {
    let mut app = playground_app();

    // First run via the REAL intent path: a sidebar tap on the first row
    // (complex-animation — the showcase manifest's alphabetical head).
    let (cx, cy) = qk_center(&app, &["row", "complex-animation"]);
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        wait_for_state(&app, "ready"),
        "the sidebar tap never ran the case: {:?}",
        app.query_text(&["app-state"])
    );

    // Second run via the probe: the swap destroys the first child and
    // spawns a fresh one under a new controller; the new child must reach
    // ready on its own rail.
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
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
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
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
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));

    // Split (the boot mode): both panes share the row.
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert!(
        editor_w > 300.0 && viewer_w > 300.0,
        "split panes: {editor_w} / {viewer_w}"
    );

    // Edit: the viewer shells into a zero-width, hard-clipped container
    // (it stays mounted — presence unchanged, width 0) and the editor
    // takes the whole row.
    click_qk(&mut app, &["tab-edit"]);
    app.wait_for_timeout(Duration::ZERO);
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert!(
        editor_w > 900.0,
        "the editor fills the row in edit mode: {editor_w}"
    );
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
    assert!(
        viewer_w > 900.0,
        "the viewer fills the row in view mode: {viewer_w}"
    );

    // Back to split.
    click_qk(&mut app, &["tab-split"]);
    app.wait_for_timeout(Duration::ZERO);
    let (editor_w, viewer_w) = (
        qk_width(&app, &["editor-pane"]).expect("the editor pane"),
        qk_width(&app, &["viewer-pane"]).expect("the viewer pane"),
    );
    assert!(
        editor_w > 300.0 && viewer_w > 300.0,
        "split again: {editor_w} / {viewer_w}"
    );
}

/// The Edit→Split wasm trap's native scenario pin (phase 6.5): every
/// Edit↔Split transition re-cases the viewer-slot Switch — the live child
/// retires and a fresh one spawns — and the status rail must settle back to
/// `ready` with the viewer hosting a live child after each swap. (The
/// wasm-only race itself — the retiring child's load RPC still in flight
/// when its worker exits — is pinned at the seam by
/// `rut_boot::rut_child_load_against_a_destroyed_child_reports_worker_gone`;
/// native loads block on the spawn control, so the trap cannot fire here.)
#[test]
fn playground_edit_split_swaps_with_a_live_child_settle_back_to_ready() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));

    for tab in ["tab-edit", "tab-split", "tab-edit", "tab-split"] {
        click_qk(&mut app, &[tab]);
        assert!(
            wait_for_state(&app, "ready"),
            "{tab}: the swap never settled back to ready: {:?}",
            app.query_text(&["app-state"])
        );
        let node = app
            .dev_tool_get_element(app.query_element(&["viewer"]).unwrap())
            .expect("the viewer host in the dev-tool tree");
        assert_eq!(
            node.name, "tur_virtual_app",
            "{tab}: the viewer hosts the child"
        );
    }
}

// ---- Phase C: reset -----------------------------------------------------------

#[test]
fn playground_reset_restores_the_original_source() {
    let mut app = playground_app();
    let original = case_source("counter");
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(
        editor_text(&app),
        original,
        "the editor loaded the case source"
    );
    assert!(
        app.query_element(&["edited-pill"]).is_none(),
        "not edited after a clean load"
    );

    // Type a keystroke: the text diverges (the edited pill appears).
    focus_editor(&mut app);
    app.send_key("x");
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["edited-pill"]).is_some(),
        "the edited pill after a keystroke"
    );

    // Reset: the original source returns and the case re-runs (the pill
    // clears — the compiled baseline moved back to the original).
    click_qk(&mut app, &["reset"]);
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(
        editor_text(&app),
        original,
        "Reset restored the case source"
    );
    assert!(
        app.query_element(&["edited-pill"]).is_none(),
        "the edited pill cleared after the reset re-ran"
    );
}

// ---- Phase C: auto-run + the edit rail ----------------------------------------

#[test]
fn playground_auto_run_respawns_after_the_debounce() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));

    // Auto-run is ON from boot (the boa default).
    assert_eq!(
        app.query_text(&["auto-state"]).as_deref(),
        Some("auto-run on")
    );

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
        app.query_text(&["compiled-ago"])
            .is_some_and(|s| s.starts_with("compiled ")),
        "the compiled-ago label: {:?}",
        app.query_text(&["compiled-ago"])
    );
}

#[test]
fn playground_auto_run_off_keeps_the_case_running() {
    let mut app = playground_app();
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));

    // Toggle auto-run OFF (the toolbar pill — the caption flips to the
    // "⌘S to run" hint).
    click_qk(&mut app, &["autorun"]);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["auto-state"]).as_deref(),
        Some("\u{2318}S to run")
    );

    // A keystroke still marks the editor edited... but never respawns:
    // the case keeps running well past the debounce window.
    focus_editor(&mut app);
    app.send_key("x");
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["edited-pill"]).is_some(),
        "the edited pill appears"
    );
    app.wait_for_timeout(Duration::from_secs(1));
    assert_eq!(
        app.query_text(&["app-state"]).as_deref(),
        Some("ready"),
        "auto-run off: no debounced recompile"
    );

    // Toggle back on (the pill flips in the status bar).
    click_qk(&mut app, &["autorun"]);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["auto-state"]).as_deref(),
        Some("auto-run on")
    );
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
    let input = Input().controller(ctrl).undo(undo_new()).width_height(400.0, 200.0)
        .multiline(true).query_key("editor").build();
    mount(Column().child(input).build());
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
    let app =
        TurTestApp::new_with_extra_plugins(600.0, 400.0, vec![Box::new(TurRutPlaygroundPlugin)])
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
    let has = |text: &str, color: u64| spans.iter().any(|(t, c)| t == text && *c == color);
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
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));

    // The case-load rail highlighted the editor: keyword, string and
    // comment runs ride the controller's span tree.
    let spans = editor_spans(&app);
    let source = joined(&spans);
    assert_eq!(
        source,
        case_source("counter"),
        "the runs tile the case source"
    );
    assert!(
        spans
            .iter()
            .any(|(t, c)| t == "entry" && *c == CODE_KEYWORD),
        "the keyword run: {spans:?}"
    );
    assert!(spans.iter().any(|(t, c)| t == "let" && *c == CODE_KEYWORD));
    assert!(
        spans
            .iter()
            .any(|(t, c)| t == "f\"Count: {" && *c == CODE_STRING),
        "the f-string run"
    );
    assert!(
        spans.iter().any(|(_, c)| *c == CODE_COMMENT),
        "a comment run"
    );

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
    assert_eq!(
        joined(&edited).len(),
        source.len() + 1,
        "the keystroke landed"
    );
    assert!(
        edited.iter().any(|(_, c)| *c == CODE_KEYWORD),
        "the keyword runs survived the keystroke"
    );
    assert!(
        edited.iter().any(|(_, c)| *c == CODE_COMMENT),
        "the comment runs survived the keystroke"
    );
    assert!(
        edited.len() > 4,
        "the span tree did not collapse: {edited:?}"
    );

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
    assert!(
        rerun
            .iter()
            .any(|(t, c)| t == "entry" && *c == CODE_KEYWORD)
    );
    assert!(rerun.iter().any(|(_, c)| *c == CODE_COMMENT));
    assert!(
        rerun
            .iter()
            .any(|(t, c)| t == "f\"Count: {" && *c == CODE_STRING)
    );
}

// ---- Phase C: the `input_on_input` row (native pin) ----------------------------

/// A minimal fixture: one Input on a realm controller wired through the
/// kit's `.on_input(name, id)` (the `input_on_input` row). Every user
/// edit delivers the named entry with the row's id crossing.
const INPUT_ON_INPUT_MODULE: &str = r#"
use tur::{ ctx_bridge, mount, tctrl_new, undo_new };
use tur_kit::{ Column, Input, InputEvent, MutationCtx, Readable, Text, mutate_input, source_str };

entry fn start() -> u64 {
    let text: Readable<str> = source_str("cold");
    let ctrl = tctrl_new();
    // The edit intent: a mutation over the typed InputEvent (the row id
    // is gone — the handler names the source by capture).
    let b_edit = mutate_input(fn (ctx: MutationCtx, _ev: InputEvent) {
        ctx.set_str(text, "edit");
    });
    let input = Input().controller(ctrl).undo(undo_new()).width_height(400.0, 200.0)
        .on_input(b_edit).query_key("input").build();
    let col = Column().child(input).child(Text().text_bound(text).query_key("echo").build());
    mount(col.build());
    return text.atom_id();
}
"#;

#[test]
fn input_on_input_row_fires_the_edit_intent() {
    let mut app = TurTestApp::new(600.0, 400.0).unwrap();
    app.load_rut_module(INPUT_ON_INPUT_MODULE).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let _text_atom = app.rut_start_answer();
    assert_eq!(
        app.query_text(&["echo"]).as_deref(),
        Some("cold"),
        "the pre-edit echo"
    );

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
        Some("edit"),
        "the on-input mutation delivered the edit"
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
        .query_element(&["row", "counter"])
        .expect("the showcase row's query key");
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
    app.call_rut_entry("select", case_index("counter"), 0.0)
        .unwrap();
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
    app.call_rut_entry("select", case_index("todolist"), 0.0)
        .unwrap();
    assert!(wait_for_state(&app, "ready"));
    assert_eq!(app.query_text(&["case-name"]).as_deref(), Some("todolist"));
}

#[test]
fn playground_divider_drag_resizes_and_clamps_the_sidebar() {
    let mut app = playground_app();

    // Boot width (the atom's seed): 200 — the clamp floor (the boa seed).
    let w0 = qk_width(&app, &["sidebar"]).expect("the sidebar");
    assert_eq!(w0, 200.0, "the seeded sidebar width");

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
    assert!(
        (w1 - 300.0).abs() < 2.0,
        "the drag moved the width 200→{w1}"
    );

    // Post-release moves over the strip must NOT resize (the drag flag
    // gates `div_move`).
    let (dx, dy) = divider_center(&app);
    app.pointer_move(dx + 40.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    let w2 = qk_width(&app, &["sidebar"]).expect("the sidebar");
    assert!(
        (w2 - w1).abs() < 0.5,
        "a hover move after release is inert: {w1}→{w2}"
    );

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
    assert_eq!(qk_width(&app, &["sidebar"]), Some(200.0), "the lower clamp");
}

// ---- Phase 4 (P0): the editor↔viewer divider -----------------------------------
// The boa shell's second VDivider: visible (and draggable) only in split
// mode, riding an editor-width atom (seed 600, clamped 360–900) — the
// editor pane goes `width_bound(edw)` in split mode, the viewer stays
// Expanded.

#[test]
fn playground_editor_divider_drags_and_clamps_the_editor_width() {
    let mut app = playground_app();

    // Split boot: the second divider sits between the editor pane and the
    // viewer pane, an 8px strip like the sidebar divider; the editor pane
    // carries the seeded width (600), the viewer fills the rest.
    let div2_center = |app: &TurTestApp| -> (f64, f64) {
        let d = app
            .query_element(&["divider2"])
            .expect("the editor↔viewer divider");
        let b = app
            .get_element_absolute_bounds(ElementNodeId::new(d.as_u64()))
            .unwrap();
        assert_eq!(b.right - b.left, 8.0, "the grab strip is 8px");
        (b.left + 4.0, (b.top + b.bottom) / 2.0)
    };
    let editor_w0 = qk_width(&app, &["editor-pane"]).expect("the editor pane");
    let viewer_w0 = qk_width(&app, &["viewer-pane"]).expect("the viewer pane");
    assert_eq!(editor_w0, 600.0, "the seeded editor width");
    assert!(viewer_w0 > 300.0, "the viewer fills the rest: {viewer_w0}");

    // Hovering the strip shows the col-resize cursor.
    let (dx, dy) = div2_center(&app);
    app.pointer_move(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::ColResize),
        "the editor divider's cursor"
    );

    // Drag right by 120: the editor pane widens 1:1, the viewer shrinks.
    app.pointer_down(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    for step in [40.0, 80.0, 120.0] {
        app.pointer_move(dx + step, dy);
        app.wait_for_timeout(Duration::ZERO);
    }
    app.pointer_up(dx + 120.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    let editor_w1 = qk_width(&app, &["editor-pane"]).expect("the editor pane");
    let viewer_w1 = qk_width(&app, &["viewer-pane"]).expect("the viewer pane");
    assert!(
        (editor_w1 - 720.0).abs() < 2.0,
        "the drag moved the editor 600→{editor_w1}"
    );
    assert!(
        (viewer_w0 - viewer_w1 - 120.0).abs() < 3.0,
        "the viewer gave the pixels back"
    );

    // Clamp high: a huge drag pins the editor at 900.
    let (dx, dy) = div2_center(&app);
    app.pointer_down(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    for step in [100.0, 300.0, 700.0] {
        app.pointer_move(dx + step, dy);
        app.wait_for_timeout(Duration::ZERO);
    }
    app.pointer_up(dx + 700.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        qk_width(&app, &["editor-pane"]),
        Some(900.0),
        "the upper clamp"
    );

    // Clamp low: a huge leftward drag pins at 360.
    let (dx, dy) = div2_center(&app);
    app.pointer_down(dx, dy);
    app.wait_for_timeout(Duration::ZERO);
    for step in [-300.0, -600.0, -1200.0] {
        app.pointer_move(dx + step, dy);
        app.wait_for_timeout(Duration::ZERO);
    }
    app.pointer_up(dx - 1200.0, dy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        qk_width(&app, &["editor-pane"]),
        Some(360.0),
        "the lower clamp"
    );

    // The divider hides outside split mode (the boa Condition): Edit shells
    // the viewer, View shells the editor — no grab strip in either.
    click_qk(&mut app, &["tab-edit"]);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["divider2"]).is_none(),
        "no divider2 in edit mode"
    );
    click_qk(&mut app, &["tab-view"]);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["divider2"]).is_none(),
        "no divider2 in view mode"
    );
    click_qk(&mut app, &["tab-split"]);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["divider2"]).is_some(),
        "divider2 back in split"
    );
}

// ---- Phase 4 (P0): comment-span metrics pin -------------------------------------
// The "wide inter-word gaps in editor comments" report: measured against
// the boa reference, the advances are identical (one monospace cell per
// char, spaces included — verified in the browser at 1× and in the layout
// stops). This pin holds the law: inside a highlighted editor, caret
// advances across a comment run equal the code runs' cell advance, so any
// future span/shaping regression trips here.

const SPACING_ROWS_MODULE: &str = r#"
use tur::{ mount, pg_apply_highlight, pg_highlight, st_put, tctrl_new, tctrl_set_text, undo_new };
use tur_kit::{ Column, Input };

let K_CTRL: u64 = 2;

entry fn start() -> u64 {
    let ctrl = tctrl_new();
    st_put(K_CTRL, ctrl);
    let src = "entry fn start() { // the quick brown fox jumps over the lazy dog\n    let x = 1; // spaced — out\n}\n";
    tctrl_set_text(ctrl, src);
    pg_apply_highlight(ctrl, pg_highlight(src));
    mount(Column().child(
        Input().controller(ctrl).undo(undo_new()).width_height(700.0, 200.0)
            .font_family("monospace").font_size(13.0)
            .multiline(true).query_key("editor").build()).build());
    return 0;
}
"#;

#[test]
fn editor_comment_spans_keep_uniform_monospace_advances() {
    let app =
        TurTestApp::new_with_extra_plugins(800.0, 400.0, vec![Box::new(TurRutPlaygroundPlugin)])
            .unwrap();
    app.load_rut_module(SPACING_ROWS_MODULE).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let _ = app.rut_start_answer();
    app.wait_for_timeout(Duration::ZERO);

    let editable = editor_editable(&app);
    let text = editor_text(&app);
    let line0_len = text.split('\n').next().map(|l| l.len()).unwrap_or(0);
    assert!(line0_len > 40, "the fixture's first line is comment-heavy");

    // Caret x at every char boundary of line 0; in a monospace face every
    // delta is one glyph advance — comment runs included.
    let mut xs: Vec<f32> = Vec::new();
    let mut b = 0usize;
    while b <= line0_len {
        let x = app
            .with_element(editable, move |e| {
                e.cast::<EditableTextElement>().unwrap().cursor_x_at(b)
            })
            .unwrap_or(None)
            .unwrap_or(-1.0);
        assert!(x >= 0.0, "caret x missing at byte {b}");
        xs.push(x);
        b += text[b..].chars().next().unwrap().len_utf8();
    }
    let deltas: Vec<f32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    let first = deltas[0];
    for (i, d) in deltas.iter().enumerate() {
        assert!(
            (d - first).abs() < 0.35,
            "non-uniform advance at char {i} of line 0: {d} vs {first} (deltas {deltas:?})"
        );
    }
}

// ---- the va-child animation surface (documented) -------------------------------
//
// The child instance's elements are invisible to the parent harness
// (query_element / query_text / dev_tool_get_element on the `tur_virtual_app`
// host see no children — the child's batch replays into the parent's PAINT
// only), so there is no headless probe of a child-side animated prop. The
// motion contract is pinned at ROOT level instead
// (`element/animation.rs::complex_animation_case_runs_the_card_studio`).
//
// Browser status of the old "va-child controller animation never advances"
// spike regression: the upgraded studio demonstrably animates in the wasm
// viewer (Play → FORWARD badge, the % readout climbing, the card tweening to
// COMPLETED/100%/W_MAX/coral — verified against the boa side-by-side), so the
// phase-5 observation ("complex-animation targets never appear") described
// the OLD bare-opacity case, whose only visible channel was a progress-0
// bound opacity. `implicit-animations` (the other named case) drives no
// visible prop from its tick, so it neither confirms nor refutes a residual
// child-animation gap.
#[test]
fn playground_complex_animation_studio_boots_to_ready() {
    // The upgraded studio case through the real spawn path: the sidebar
    // select compiles the new source (embedded via cases_gen) and the
    // viewer hosts the live child. (Motion itself is pinned at root level
    // by the corpus test; the browser side-by-side verified the in-viewer
    // tween end to end.)
    let mut app = playground_app();
    let (cx, cy) = qk_center(&app, &["row", "complex-animation"]);
    app.click(cx, cy);
    assert!(
        wait_for_state(&app, "ready"),
        "the upgraded complex-animation never reached ready: {:?}",
        app.query_text(&["app-state"])
    );
    let node = app
        .dev_tool_get_element(app.query_element(&["viewer"]).unwrap())
        .expect("the viewer host in the dev-tool tree");
    assert_eq!(node.name, "tur_virtual_app");
    assert!(
        node.size.0 > 0.0 && node.size.1 > 0.0,
        "the hosting element kept its layout: {:?}",
        node.size
    );
}
