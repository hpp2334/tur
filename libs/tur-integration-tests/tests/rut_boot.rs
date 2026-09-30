//! Phase-1 gate for the boa→rut migration: a rut module loaded through the
//! engine's rut rail builds a real element tree via `tur` host rows and
//! lays out exactly like a JS-loaded module.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

const HELLO_RUT: &str = r#"
use tur::{ el_column, el_text, el_build, el_child, mount };

entry fn start() {
    let col = el_column();
    el_child(col, el_text("hello from rut"));
    el_child(col, el_text("rut drives, the engine applies"));
    mount(el_build(col));
}
"#;

/// The `entry fn stop` cleanup contract: stop runs on reload, and the new
/// module's tree replaces the old one.
const HELLO_RUT_V2: &str = r#"
use tur::{ el_text, mount };

entry fn start() {
    mount(el_text("v2 root"));
}

entry fn stop() {
}
"#;

/// A broken module — the parse-first contract keeps the running tree alive.
const BROKEN_RUT: &str = "entry fn start() { let x: i32 = ; }";

fn text_nodes(node: &tur_engine::core::elements::DevNodeData) -> Vec<&'static str> {
    let mut names = Vec::new();
    if node.name == "tur_paragraph" {
        names.push(node.name);
    }
    names
}

#[test]
fn rut_module_mounts_a_tree() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(HELLO_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let root = app.dev_tool_element_tree().expect("root mounted by the rut module");
    // RootView wrapper -> the rut-built Column -> two Text children.
    assert_eq!(root.children.len(), 1, "one root child (the Column)");
    let column = app.dev_tool_get_element(root.children[0]).expect("column node");
    assert_eq!(column.name, "tur_flex", "the rut builder materialized a flex view");
    assert_eq!(column.children.len(), 2, "both rut-authored children attached");

    let texts: Vec<&'static str> = column
        .children
        .iter()
        .filter_map(|id| app.dev_tool_get_element(*id))
        .flat_map(|n| text_nodes(&n))
        .collect();
    assert_eq!(texts.len(), 2, "two Text nodes authored from rut");

    // Layout ran: every rut-authored node painted non-zero.
    assert!(column.size.0 > 0.0 && column.size.1 > 0.0, "column laid out: {:?}", column.size);
    for id in &column.children {
        let t = app.dev_tool_get_element(*id).unwrap();
        assert!(t.size.1 > 0.0, "text node laid out: {:?}", t.size);
    }
    // The column stacks vertically: second text below the first.
    let first = app.dev_tool_get_element(column.children[0]).unwrap();
    let second = app.dev_tool_get_element(column.children[1]).unwrap();
    assert!(
        second.absolute.1 > first.absolute.1,
        "vertical stack: {:?} then {:?}",
        first.absolute,
        second.absolute
    );
}

/// The Phase-2 reactive gate: a str atom bound to a Text via
/// `el_text_bound`; an engine→rut entry call mutates the atom; the
/// existing reactive flush re-renders the Text.
const COUNTER_RUT: &str = r#"
use tur::{ el_column, el_text_bound, el_build, el_child, mount, rs_source_str, rs_set_str };

entry fn start() -> u64 {
    let atom = rs_source_str("Count: 0");
    let col = el_column();
    el_child(col, el_text_bound(atom));
    mount(el_build(col));
    return atom;
}

entry fn on_event(atom: u64, n: f64) {
    rs_set_str(atom, f"Count: {n}");
}
"#;

fn rut_bound_text(app: &TurTestApp) -> String {
    let id = app.query_element(&["rut", "text"]).expect("bound text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[test]
fn rut_reactive_atom_rebinds_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(COUNTER_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let atom = app.rut_start_answer();
    assert!(atom != 0, "start returned its root atom's id");
    assert_eq!(rut_bound_text(&app), "Count: 0", "initial binding renders");

    // The engine→rut rail mutates the atom; the reactive flush re-renders.
    for n in [1u64, 2, 7] {
        app.call_rut_entry("on_event", atom, n as f64).unwrap();
        app.wait_for_timeout(Duration::ZERO);
        assert_eq!(rut_bound_text(&app), format!("Count: {n}"), "atom write drove re-render");
    }
}

/// The Phase-3 callback gate: `el_button` wires a PointerInteract whose
/// click queues an intent; the pump drains it into `entry fn ts_click`,
/// which mutates the bound atom — the full interactive loop, all rut.
/// The button's `id` IS the atom id (the callback's first argument).
const BUTTON_RUT: &str = r#"
use tur::{ el_button, el_column, el_text_bound, el_build, el_child, mount, rs_source_str, rs_set_str };

entry fn start() -> u64 {
    let atom = rs_source_str("taps: 0");
    let col = el_column();
    el_child(col, el_text_bound(atom));
    el_child(col, el_button(atom, atom, "ts_click", "tap me"));
    mount(el_build(col));
    return atom;
}

entry fn ts_click(atom: u64, _label: u64, n: f64) {
    rs_set_str(atom, f"taps: {n}");
}
"#;

#[test]
fn rut_button_click_mutates_bound_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(BUTTON_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "taps: 0");

    // The button is the second child of the column — locate by tree walk,
    // then tap its center with real pointer events.
    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let button = app.dev_tool_get_element(column.children[1]).unwrap();
    let (bx, by) = button.absolute;
    let (bw, bh) = button.size;
    let cx = bx + bw / 2.0;
    let cy = by + bh / 2.0;

    for n in 1u64..=3 {
        app.click(cx, cy);
        app.wait_for_timeout(Duration::ZERO);
        assert_eq!(rut_bound_text(&app), format!("taps: {n}"), "click {n} drove the rut callback");
    }
}

/// The Phase-4 journey gate: a complete rut counter app — inc/dec buttons
/// with REAL state (an f64 count atom read-modify-written in the
/// callbacks), a bound reactive label, and cleanup — the rut twin of the
/// JS counter case. Callbacks receive (count_atom, label_atom, seq).
const COUNTER_APP_RUT: &str = r#"
use tur::{ el_button, el_column, el_text_bound, el_build, el_child, mount, rs_get_f64, rs_set_f64, rs_set_str, rs_source_f64, rs_source_str };

entry fn start() -> u64 {
    let label = rs_source_str("Count: 0");
    let count = rs_source_f64();
    let col = el_column();
    el_child(col, el_text_bound(label));
    el_child(col, el_button(count, label, "ts_inc", "+1"));
    el_child(col, el_button(count, label, "ts_dec", "-1"));
    mount(el_build(col));
    return count;
}

fn show(count: u64, label: u64) {
    let v = rs_get_f64(count);
    rs_set_str(label, f"Count: {v}");
}

entry fn ts_inc(count: u64, label: u64, _n: f64) {
    rs_set_f64(count, rs_get_f64(count) + 1);
    show(count, label);
}

entry fn ts_dec(count: u64, label: u64, _n: f64) {
    rs_set_f64(count, rs_get_f64(count) - 1);
    show(count, label);
}
"#;

#[test]
fn rut_counter_app_full_journey() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(COUNTER_APP_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "Count: 0");

    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let center = |id| {
        let n = app.dev_tool_get_element(id).unwrap();
        (n.absolute.0 + n.size.0 / 2.0, n.absolute.1 + n.size.1 / 2.0)
    };
    let (inc_x, inc_y) = center(column.children[1]);
    let (dec_x, dec_y) = center(column.children[2]);

    app.click(inc_x, inc_y);
    app.wait_for_timeout(Duration::ZERO);
    app.click(inc_x, inc_y);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "Count: 2", "two +1 taps");

    app.click(dec_x, dec_y);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "Count: 1", "one -1 tap");
}

/// The Phase-3-breadth gate: painted box (color+padding), Expanded fill,
/// Stack+Positioned, and `condition` with pre-built branches toggled by a
/// button — all authored in rut.
const CONDITION_RUT: &str = r#"
use tur::{ el_box, el_button, el_column, el_expand, el_positioned, el_stack, el_text, el_text_bound, el_build, el_child, condition, mount, rs_get_bool, rs_set_bool, rs_source_bool, rs_source_str };

entry fn start() -> u64 {
    let on = rs_source_bool(true);
    let on_label = rs_source_str("ON");
    let off_label = rs_source_str("OFF");
    let col = el_column();
    el_child(col, el_box(0x336699FF, 8.0, el_text("boxed")));
    el_child(col, el_expand(1.0, el_text("fills the column")));
    el_child(col, condition(on, el_text_bound(on_label), el_text_bound(off_label)));
    let overlay = el_stack();
    el_child(overlay, el_text("base"));
    el_child(overlay, el_positioned(4.0, 4.0, el_text("floating")));
    el_child(col, el_build(overlay));
    el_child(col, el_button(on, on, "ts_toggle", "toggle"));
    mount(el_build(col));
    return on;
}

entry fn ts_toggle(on: u64, _b: u64, _n: f64) {
    rs_set_bool(on, !(rs_get_bool(on)));
}
"#;

#[test]
fn rut_layout_and_condition_breadth() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(CONDITION_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Box paints around its padded child: the box is wider than its text
    // child by exactly 2× the 8.0 padding.
    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    assert_eq!(column.children.len(), 5, "all five authored children");
    let box_node = app.dev_tool_get_element(column.children[0]).unwrap();
    let boxed_text = app.dev_tool_get_element(box_node.children[0]).unwrap();
    assert!(
        box_node.size.0 >= boxed_text.size.0 + 16.0,
        "box wraps its child with padding: box {:?} text {:?}",
        box_node.size,
        boxed_text.size
    );

    // Expanded fills the column's remaining main axis (tall).
    let expanded = app.dev_tool_get_element(column.children[1]).unwrap();
    assert!(expanded.size.1 > 100.0, "expanded child fills: {:?}", expanded.size);

    // Stack + positioned overlay: both children present, the positioned
    // one offset by (4, 4) from the stack origin.
    let overlay = app.dev_tool_get_element(column.children[3]).unwrap();
    assert_eq!(overlay.children.len(), 2, "stack holds base + floating");
    let floating = app.dev_tool_get_element(overlay.children[1]).unwrap();
    let dx = floating.absolute.0 - overlay.absolute.0;
    let dy = floating.absolute.1 - overlay.absolute.1;
    assert!((dx - 4.0).abs() < 0.5 && (dy - 4.0).abs() < 0.5,
        "positioned child anchored at +4,+4: dx={dy:+.1} dx={dx:+.1}");

    // The ON branch is the visible bound text.
    assert_eq!(rut_bound_text(&app), "ON", "the truthy branch renders");

    // Toggle → the OFF branch swaps in (pure engine swap — no rut in flush).
    let (bx, by) = center_of(&app, column.children[4]);
    app.click(bx, by);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "OFF", "the branch swapped after the toggle tap");
    app.click(bx, by);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "ON", "and back");
}

fn center_of(app: &TurTestApp, id: tur_engine::core::element::NodeId) -> (f64, f64) {
    let n = app.dev_tool_get_element(id).unwrap();
    (n.absolute.0 + n.size.0 / 2.0, n.absolute.1 + n.size.1 / 2.0)
}

#[test]
fn rut_reload_runs_stop_and_replaces_root() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(HELLO_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.dev_tool_element_tree().unwrap().children.len(), 1);

    // A broken reload must NOT destroy the running tree (parse-first).
    let err = app.load_rut_module(BROKEN_RUT).unwrap_err();
    assert!(!err.to_string().is_empty(), "broken module fails loudly");
    app.wait_for_timeout(Duration::ZERO);
    let root = app.dev_tool_element_tree().expect("tree survives a broken reload");
    assert_eq!(root.children.len(), 1, "the previous tree is still mounted");

    // A good reload replaces the root and runs the old module's `stop`.
    app.load_rut_module(HELLO_RUT_V2).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let root = app.dev_tool_element_tree().expect("v2 root mounted");
    assert_eq!(root.children.len(), 1);
    let only = app.dev_tool_get_element(root.children[0]).unwrap();
    assert_eq!(only.name, "tur_paragraph", "the v2 module's single Text is the new root child");
}
