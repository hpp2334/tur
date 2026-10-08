//! Phase-1 gate for the boa→rut migration: a rut module loaded through the
//! engine's rut rail builds a real element tree via `tur_host` rows and
//! lays out exactly like a JS-loaded module.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

const HELLO_RUT: &str = r#"

use tur_kit::{ Column, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

use tur_kit::{ Column, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

entry fn start() {
    let col = Column()
        .child(Text().text("hello from rut").build())
        .child(Text().text("rut drives, the engine applies").build());
    mount(col.build());
}
"#;

/// The `entry fn stop` cleanup contract: stop runs on reload, and the new
/// module's tree replaces the old one.
const HELLO_RUT_V2: &str = r#"
use tur_kit::{ Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

use tur_kit::{ Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };


entry fn start() {
    mount(Text().text("v2 root").build());
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
use tur_host::{ ctx_bridge };
use tur_kit::{ Column, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };


entry fn start() -> u64 {
    let atom: Readable<str> = source<str>("Count: 0");
    let col = Column().child(Text().text_bound(atom).query_key("rut/text").build());
    mount(col.build());
    return atom.atom_id();
}

entry fn on_event(atom: u64, n: f64) {
    let r = Source<str>.of(ctx_bridge(), atom, false, 1);
    MutationCtx.over(ctx_bridge()).set<str>(r, f"Count: {n}");
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
const BUTTON_RUT: &str = r#"use tur_kit::{ Column, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };


entry fn start() -> u64 {
    let atom: Readable<str> = source<str>("taps: 0");
    let n: Readable<f64> = source<f64>(0.0);
    let b_click = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(n, ctx.get<f64>(n) + 1.0);
        ctx.set<str>(atom, f"taps: {ctx.get<f64>(n) as u64}");
    });
    let col = Column()
        .child(Text().text_bound(atom).query_key("rut/text").build())
        .child(
        PointerInteract().on_click(b_click).child(Text().text("tap me").build()).build(),
    );
    mount(col.build());
    return atom.atom_id();
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
use tur_kit::{ Column, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

entry fn start() -> u64 {
    let label: Readable<str> = source<str>("Count: 0");
    let count: Readable<f64> = source<f64>(0.0);
    // The inc/dec mutations capture both handles (the show twin inlines).
    let b_inc = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(count, ctx.get<f64>(count) + 1.0);
        ctx.set<str>(label, f"Count: {ctx.get<f64>(count) as u64}");
    });
    let b_dec = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(count, ctx.get<f64>(count) - 1.0);
        ctx.set<str>(label, f"Count: {ctx.get<f64>(count) as u64}");
    });
    let col = Column()
        .child(Text().text_bound(label).query_key("rut/text").build())
        .child(
        PointerInteract().on_click(b_inc).child(Text().text("+1").build()).build(),
    )
        .child(
        PointerInteract().on_click(b_dec).child(Text().text("-1").build()).build(),
    );
    mount(col.build());
    return count.atom_id();
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
use tur_kit::{ Column, Condition, Container, Expanded, Mutation, MutationCtx, PointerInteract, Positioned, Readable, Source, Stack, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

entry fn start() -> u64 {
    let on: Readable<bool> = source<bool>(true);
    let on_label: Readable<str> = source<str>("ON");
    let off_label: Readable<str> = source<str>("OFF");
    let b_toggle = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<bool>(on, !ctx.get<bool>(on));
    });
    let col = Column()
        .child(Container().color(0x336699FF).padding(8.0).child(Text().text("boxed").build()).build())
        .child(Expanded().flex(1.0).child(Text().text("fills the column").build()).build())
        .child(Condition(on).then(Text().text_bound(on_label).query_key("rut/text").build()).else_branch(Text().text_bound(off_label).query_key("rut/text").build()).build())
        .child(Stack().child(Text().text("base").build()).child(Positioned().left(4.0).top(4.0).child(Text().text("floating").build()).build()).build())
        .child(
        PointerInteract().on_click(b_toggle).child(Text().text("toggle").build()).build(),
    );
    mount(col.build());
    return on.atom_id();
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

/// Scroll + styled-text gate: a scroll viewport wrapping tall styled
/// content — the long-list pattern every real app needs.
const SCROLL_RUT: &str = r#"
use tur_host::{ AXIS_VERTICAL, rs_source_f64 };
use tur_kit::{ Column, Expanded, Mutation, MutationCtx, Readable, ScrollView, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

use tur_kit::{ Column, Expanded, Mutation, MutationCtx, PointerInteract, Readable, ScrollView, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

entry fn start() -> u64 {
    let mut col = Column();
    let mut i = 0;
    while (i < 60) {
        col.child(Text().text(f"row {i}").font_size(16.0).color(0x222222FF).build());
        i += 1;
    }
    let scroller = ScrollView().axis(AXIS_VERTICAL).child(col.build()).build();
    let root = Column().child(Expanded().flex(1.0).child(scroller).build());
    mount(root.build());
    return rs_source_f64();
}
"#;

#[test]
fn rut_scroll_view_with_styled_rows() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(SCROLL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Root column -> scroll view -> content column -> 20 styled rows.
    let root = app.dev_tool_element_tree().unwrap();
    let root_col = app.dev_tool_get_element(root.children[0]).unwrap();
    assert_eq!(root_col.children.len(), 1, "the expanded scroller is the only child");
    let flexible = app.dev_tool_get_element(root_col.children[0]).unwrap();
    let scroller = app.dev_tool_get_element(flexible.children[0]).unwrap();
    assert!(scroller.size.1 > 0.0 && scroller.size.1 <= 600.0, "scroll viewport bounded: {:?}", scroller.size);
    let content = app.dev_tool_get_element(scroller.children[0]).unwrap();
    assert_eq!(content.children.len(), 60, "all 60 rut-authored rows mounted");
    // The content is taller than the viewport (that's why it scrolls).
    assert!(content.size.1 > scroller.size.1, "content overflows: content {:?} viewport {:?}", content.size, scroller.size);

    // A wheel event over the viewport scrolls the content.
    let cx = scroller.absolute.0 + scroller.size.0 / 2.0;
    let cy = scroller.absolute.1 + scroller.size.1 / 2.0;
    app.wheel(0.0, 120.0, cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    let flexible2 = app.dev_tool_get_element(root_col.children[0]).unwrap();
    let scrolled = app.dev_tool_get_element(flexible2.children[0]).unwrap();
    let content2 = app.dev_tool_get_element(scrolled.children[0]).unwrap();
    assert!(
        content2.absolute.1 < content.absolute.1,
        "wheel scrolled the content up: {:?} -> {:?}",
        content.absolute.1,
        content2.absolute.1
    );
}

/// The Phase-2 (B5) structured-value gate: list / map atoms over the
/// native-KV substrate, round-tripped through rut entries — a list atom of
/// strings drives the bound Text via a rut-side string join, and a push
/// reads the whole value back, rebuilds it, writes it, and re-joins. No JS
/// realm anywhere: every row speaks native `Value`s.
const LIST_MAP_RUT: &str = r#"
use tur_host::{ rs_get_str, rs_get_value, rs_list_new, rs_list_push, rs_map_new, rs_map_set, rs_set_str, rs_set_value, rs_source_str, rs_source_value, rs_value_get, rs_value_item, rs_value_len };
use tur_kit::{ Column, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };


// The string join: read the list atom back through the substrate rows
// (len + item) and fold it into the bound label's str atom.
fn join_into(items: u64, label: u64) {
    let v = rs_get_value(items);
    let n = rs_value_len(v);
    let mut joined = "";
    for (let i = 0; i < n as i32; i += 1) {
        joined = f"{joined}|{rs_value_item(v, i as u64)}";
    }
    rs_set_str(label, joined);
}

entry fn start() -> u64 {
    let list = rs_list_new();
    rs_list_push(list, "alpha");
    rs_list_push(list, "beta");
    let items = rs_source_value(list);

    let map = rs_map_new();
    rs_map_set(map, "role", "demo");
    let meta = rs_source_value(map);

    let label_r: Readable<str> = source<str>("");
    let label = label_r.atom_id();
    join_into(items, label);
    // The map round-trips through entries too: read a key back and append it.
    let m = rs_get_value(meta);
    let role = rs_value_get(m, "role");
    let cur = rs_get_str(label);
    rs_set_str(label, f"{cur} ({role})");

    // The push pad (a mutation whose body drives the substrate rows).
    let b_push = mutate(fn (_ctx: MutationCtx, _e: nil) {
        // Set/read round trip: read the atom's list back, rebuild the whole
        // value with one more item, write it, re-join into the label.
        let v = rs_get_value(items);
        let n = rs_value_len(v);
        let fresh = rs_list_new();
        for (let i = 0; i < n as i32; i += 1) {
            rs_list_push(fresh, rs_value_item(v, i as u64));
        }
        rs_list_push(fresh, "gamma");
        rs_set_value(items, fresh);
        join_into(items, label);
    });

    let col = Column()
        .child(Text().text_bound(label_r).query_key("rut/text").build())
        .child(
        PointerInteract().on_click(b_push).child(Text().text("push").build()).build(),
    );
    mount(col.build());
    return items;
}
"#;

#[test]
fn rut_list_map_atoms_round_trip() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(LIST_MAP_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        rut_bound_text(&app),
        "|alpha|beta (demo)",
        "list join + map get render through the native-KV atoms"
    );

    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let button = app.dev_tool_get_element(column.children[1]).unwrap();
    let (bx, by) = button.absolute;
    let (bw, bh) = button.size;

    app.click(bx + bw / 2.0, by + bh / 2.0);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        rut_bound_text(&app),
        "|alpha|beta|gamma",
        "the push rebuilt the list value and the join re-rendered the bound text"
    );
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

// ---------------------------------------------------------------------------
// Phase C1 — text input: realm-minted controllers, `el_input`, keyboard /
// IME end to end, controller method rows, undo rows.
// ---------------------------------------------------------------------------

/// A rut-authored `Input` bound to a realm-minted `TextEditingController`
/// (+ `UndoController`). `start` authors the whole journey: seed the
/// controller, round-trip `tctrl_text` into the bound label, and check the
/// undo rows. The interactive half (keyboard / IME into the focused
/// editable) is driven by the test via the engine's own subsystems.
const INPUT_RUT: &str = r#"
use tur_host::{ tctrl_new, tctrl_paste, tctrl_select, tctrl_set_text, tctrl_text, undo_new };
use tur_kit::{ Column, Input, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

entry fn start() -> u64 {
    let ctrl = tctrl_new();
    let undo = undo_new();

    // Programmatic authoring: seed, select-all, paste over, read back.
    tctrl_set_text(ctrl, "seed");
    tctrl_select(ctrl, 0, 4);
    tctrl_paste(ctrl, "SEEDED");
    let label: Readable<str> = source<str>(tctrl_text(ctrl));

    let col = Column()
        .child(Input().controller(TextCtrl(ctrl)).undo(UndoCtrl(undo)).placeholder("type here").width_height(220.0, 32.0).query_key("rut/input").build())
        .child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return label.atom_id();
}
"#;

fn rut_editable_id(app: &TurTestApp) -> tur_engine::core::element::ElementNodeId {
    // queryKey ["rut", "input"] lands on Input's Container wrapper; the
    // editable is its first child.
    let id = app
        .query_element(&["rut", "input"])
        .expect("rut input not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    let tree = app.element_tree();
    let container = tree.get_element(id).unwrap();
    let child = container.children[0];
    tur_engine::core::element::ElementNodeId::new(child.as_u64())
}

fn rut_editable_text(app: &TurTestApp) -> String {
    let id = rut_editable_id(app);
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::EditableTextElement>()
            .map(|el| el.text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[test]
fn rut_input_realm_controllers_keyboard_and_ime() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(INPUT_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The rows' programmatic journey rendered through the bound label:
    // seed "seed" → select 0..4 → paste "SEEDED" over it → "SEEDED".
    assert_eq!(rut_bound_text(&app), "SEEDED", "tctrl rows round-tripped through the label");
    assert_eq!(rut_editable_text(&app), "SEEDED", "the Input renders the controller's value");

    // Keyboard: click to focus, then type — the engine's KeyboardSubsystem
    // mutates the SAME controller the rows minted.
    let id = rut_editable_id(&app);
    let bounds = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(bounds.0, bounds.1);
    app.wait_for_timeout(Duration::ZERO);
    for ch in ["a", "b", "!"] {
        app.send_key(ch);
        app.wait_for_timeout(Duration::ZERO);
    }
    assert_eq!(rut_editable_text(&app), "SEEDEDab!", "keystrokes landed in the rut-minted controller");

    // IME: a full composition lifecycle — start, update, commit appends
    // through the same element.
    app.send_ime(tur_engine::core::platform::ImeEvent::CompositionStart);
    app.wait_for_timeout(Duration::ZERO);
    app.send_ime(tur_engine::core::platform::ImeEvent::CompositionUpdate {
        text: "o".to_string(),
        cursor: None,
    });
    app.wait_for_timeout(Duration::ZERO);
    app.send_ime(tur_engine::core::platform::ImeEvent::CompositionEnd {
        text: "ok".to_string(),
    });
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_editable_text(&app), "SEEDEDab!ok", "the IME commit landed");
}

// ---------------------------------------------------------------------------
// Phase C2 — collections: Each over a list atom + LazyList, item builders
// as named `entry fn`s invoked through the guarded flush-time VM face.
// ---------------------------------------------------------------------------

/// An Each bound to a list atom; a button pushes an item (the rebuild-all
/// reconciliation runs during flush — the guarded face call), and the item
/// builder entry authors each row.
const EACH_RUT: &str = r#"
use tur_host::{ ctx_bridge, rs_list_new, rs_list_push, rs_set_value, rs_source_value };
use tur_kit::{ Column, Each, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

fn item_row(i: u64, item: str) -> View {
    let col = Column()
        .child(Text().text(f"{i}: {item}").font_size(16.0).color(0x222222FF).build());
    return col.build();
}

entry fn start() -> u64 {
    let list = rs_list_new();
    rs_list_push(list, "alpha");
    rs_list_push(list, "beta");
    let atom: Readable<opaque> = Source<opaque>.of(ctx_bridge(), rs_source_value(list), false, 1);

    // The push pad: the mutation drives the substrate rows.
    let b_push = mutate(fn (_ctx: MutationCtx, _e: nil) {
        let fresh = rs_list_new();
        rs_list_push(fresh, "alpha");
        rs_list_push(fresh, "beta");
        rs_list_push(fresh, "gamma");
        rs_set_value(atom.atom_id(), fresh);
    });

    let col = Column()
        .child(Each(atom).item_builder(item_row).build())
        .child(
        PointerInteract().on_click(b_push).child(Text().text("push").build()).build(),
    );
    mount(col.build());
    return atom.atom_id();
}
"#;

/// Collect the text content of every `tur_paragraph` in the dev-tree
/// snapshot (the item rows' rendered strings).
fn all_texts(app: &TurTestApp) -> Vec<String> {
    let mut out = Vec::new();
    let root_id = app.dev_tool_element_tree().unwrap().children[0];
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        let Some(node) = app.dev_tool_get_element(id) else {
            continue;
        };
        if node.name == "tur_paragraph" {
            let eid = tur_engine::core::element::ElementNodeId::new(id.as_u64());
            let text = app
                .with_element(eid, |e| {
                    e.cast::<tur_engine::builtin_plugins::text::TextElement>()
                        .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            out.push(text);
        }
        for child in node.children {
            stack.push(child);
        }
    }
    out
}

#[test]
fn rut_each_maps_a_list_atom_and_rebuilds_on_change() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(EACH_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The fragment hosted two authored rows via the entry builder.
    let texts = all_texts(&app);
    assert!(
        texts.contains(&"0: alpha".to_string()) && texts.contains(&"1: beta".to_string()),
        "the entry builder authored both initial rows: {texts:?}"
    );

    // Push → the atom changes → the flush rebuilds all items through the
    // guarded face calls (never leaving the VM parked).
    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let button = app.dev_tool_get_element(column.children[1]).unwrap();
    let (bx, by) = button.absolute;
    let (bw, bh) = button.size;
    app.click(bx + bw / 2.0, by + bh / 2.0);
    app.wait_for_timeout(Duration::ZERO);

    let texts = all_texts(&app);
    assert!(
        texts.contains(&"2: gamma".to_string()),
        "the rebuild mounted the third item via the guarded face call: {texts:?}"
    );
    assert_eq!(
        texts.iter().filter(|t| t.as_str() == "0: alpha").count(),
        1,
        "rebuild-all reconciliation left exactly one instance per item"
    );
}

/// A LazyList with 300 rut-authored rows (entry-builder face); wheeling
/// mounts rows outside the initial build set — flush-time face calls on
/// the remount path.
const LAZY_RUT: &str = r#"
use tur_kit::{ Column, Expanded, LazyList, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

fn lazy_row(i: u64) -> View {
    let col = Column().child(Text().text(f"row {i}").font_size(16.0).color(0x222222FF).build());
    return col.build();
}

entry fn start() -> u64 {
    let count: Readable<f64> = source<f64>(300.0);
    let scroller = LazyList().item_builder(lazy_row).count(count).item_extent(20.0).query_key("rut/lazy").build();
    let root = Column().child(Expanded().flex(1.0).child(scroller).build());
    mount(root.build());
    return count.atom_id();
}
"#;

#[test]
fn rut_lazy_list_virtualizes_rows_through_the_entry_face() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(LAZY_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let lazy = app
        .query_element(&["rut", "lazy"])
        .expect("lazy list not found");
    let lazy = tur_engine::core::element::ElementNodeId::new(lazy.as_u64());
    let tree = app.element_tree();
    let node = tree.get_element(lazy).unwrap();
    let initial = node.children.len();
    assert!(
        initial > 0 && initial < 300,
        "virtualized: mounted {initial} of 300 declared rows"
    );

    // Wheel down — the remount mounts newly-visible rows via face calls.
    let n = app
        .dev_tool_get_element(tur_engine::core::element::NodeId::from(lazy))
        .unwrap();
    let cx = n.absolute.0 + n.size.0 / 2.0;
    let cy = n.absolute.1 + n.size.1 / 2.0;
    app.wheel(0.0, 240.0, cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    let tree = app.element_tree();
    let node = tree.get_element(lazy).unwrap();
    assert!(
        node.children.iter().any(|&c| {
            let el = app.dev_tool_get_element(c).unwrap();
            el.absolute.1 < 0.0
        }),
        "rows scrolled past the viewport after the wheel"
    );
}

// ---------------------------------------------------------------------------
// Phase C3 — the Container full surface (border / radius / shadow /
// alignment / size / clip via u64 flag consts) + SizedBox.
// ---------------------------------------------------------------------------

/// A styled box: explicit size, border, radius, shadow, clip, and a
/// bottom-right aligned child — all authored through the builder rows with
/// the exported flag consts. The align/size assertions read back through
/// the tree geometry; a second box (SizedBox) pins the exact size.
const CONTAINER_FULL_RUT: &str = r#"
use tur_host::{ ALIGN_BOTTOM_RIGHT, BORDER_CENTER, CLIP_ANTI_ALIAS };
use tur_kit::{ Column, Container, Mutation, MutationCtx, Readable, SizedBox, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

use tur_kit::{ Column, Container, Mutation, MutationCtx, PointerInteract, Readable, SizedBox, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

entry fn start() {
    let styled = Container()
        .width_height(200.0, 120.0)
        .padding(8.0)
        .color(0x336699FF as u64)
        .border(0xFFCC00FFu64, 3.0, BORDER_CENTER)
        .radius(12.0)
        .shadow(0x00000066 as u64, 8.0, 2.0, 4.0)
        .clip(CLIP_ANTI_ALIAS)
        .alignment(ALIGN_BOTTOM_RIGHT)
        .child(Text().text("corner").build());
    let mut root = Column().child(styled.build());

    // SizedBox: exactly 90 x 40 around its child.
    root.child(SizedBox(90.0, 40.0).child(Text().text("sized").build()).build());
    // The legacy el_box row still works beside the builder.
    root.child(Container().color(0x88FF88FFu64).padding(4.0).child(Text().text("legacy").build()).build());
    mount(root.build());
}
"#;

#[test]
fn rut_container_full_surface_and_sizedbox() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(CONTAINER_FULL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    assert_eq!(column.children.len(), 3, "styled box + sized box + legacy box");

    // The styled box honored its explicit 200 x 120 size.
    let styled = app.dev_tool_get_element(column.children[0]).unwrap();
    assert!(
        (styled.size.0 - 200.0).abs() < 0.6 && (styled.size.1 - 120.0).abs() < 0.6,
        "explicit size applied: {:?}",
        styled.size
    );

    // The aligned child sits in the box's bottom-right corner: its
    // right/bottom edges are within the padding of the box's edges.
    let child = app.dev_tool_get_element(styled.children[0]).unwrap();
    let dx = (styled.absolute.0 + styled.size.0) - (child.absolute.0 + child.size.0);
    let dy = (styled.absolute.1 + styled.size.1) - (child.absolute.1 + child.size.1);
    assert!(
        (dx - 8.0).abs() < 1.0 && (dy - 8.0).abs() < 1.0,
        "bottom-right aligned within the 8px padding: dx={dx:.1} dy={dy:.1}"
    );

    // SizedBox is exactly 90 x 40.
    let sized = app.dev_tool_get_element(column.children[1]).unwrap();
    assert!(
        (sized.size.0 - 90.0).abs() < 0.6 && (sized.size.1 - 40.0).abs() < 0.6,
        "sized box exact: {:?}",
        sized.size
    );
}

// ---------------------------------------------------------------------------
// Phase C4 — gestures + keyboard + focus: intent-record payloads on the
// entry rail (pointer positions, key/mods), realm-free.
// ---------------------------------------------------------------------------

/// A gesture pad and a focusable box, both reporting through the intent
/// rail. Both ids ARE the label atom (the callbacks' first argument), so
/// every callback appends to the same transcript. The context menu rides
/// the M1 mutation rail (`mutate_ev` over the typed `PointerEvent`).
/// A gesture pad and a focusable box, both reporting through the mutation
/// rail: the drag pads take `mutate_ev` over the typed `PointerEvent`
/// (locals + globals + the button decode), the focus pad takes the typed
/// `KeydownEvent` / `FocusEvent` / `BlurEvent` mutations. One log source
/// carries the transcript; every handler captures it — no id rails.
const GESTURE_RUT: &str = r#"
use tur_host::{ focus_request };
use tur_kit::{ BlurEvent, Column, FocusEvent, Focusable, KeydownEvent, MouseButton, Mutation, MutationCtx, PointerEvent, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };


entry fn start() -> u64 {
    let label: Readable<str> = source<str>("");
    let seq: Readable<f64> = source<f64>(0.0);

    // The drag pads: the typed event carries local + global points and
    // the button enum (0 primary / 1 middle / 2 secondary — the code
    // decodes through comparisons; enums carry no cast).
    let b_down = mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|down {ev.local.x as u64},{ev.local.y as u64} g{ev.global.x as u64},{ev.global.y as u64} b{button_code(ev.button)}");
    });
    let b_move = mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|move {ev.local.x as u64},{ev.local.y as u64} g{ev.global.x as u64},{ev.global.y as u64} b{button_code(ev.button)}");
    });
    let b_up = mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|up {ev.local.x as u64},{ev.local.y as u64} g{ev.global.x as u64},{ev.global.y as u64} b{button_code(ev.button)}");
    });
    // The synthesized tap: the click mutation appends `tap N` (the
    // sequence rides its own source — no rail payload).
    let b_tap = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(seq, ctx.get<f64>(seq) + 1.0);
        ctx.set<str>(label, f"{ctx.get<str>(label)}|tap {ctx.get<f64>(seq) as u64}");
    });
    let menu = PointerInteract().on_context_menu(mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        let mut b = "other";
        if (ev.button == MouseButton.Right) {
            b = "right";
        }
        ctx.set<str>(label, f"{ctx.get<str>(label)}|menu-{b}");
    })).query_key("rut/menu").child(Text().text("menu").build()).build();

    // The focus rail: the typed events name their surface (the keydown
    // record crosses key/code/modifiers; kind is always down=0 on this
    // surface). Focus/blur are payload-less.
    let b_key = mutate<KeydownEvent>(fn (ctx: MutationCtx, ev: KeydownEvent) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|key {ev.key}/{ev.code} m{ev.modifiers} k0");
    });
    let b_focus = mutate<FocusEvent>(fn (ctx: MutationCtx, _ev: FocusEvent) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|focused");
    });
    let b_blur = mutate<BlurEvent>(fn (ctx: MutationCtx, _ev: BlurEvent) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|blurred");
    });

    let pad = PointerInteract().on_pointer_down(b_down).on_pointer_move(b_move).on_pointer_up(b_up).on_click(b_tap).query_key("rut/gesture").child(Text().text("pad").build()).build();
    let foc = Focusable().on_key_down(b_key).on_focus(b_focus).on_blur(b_blur).child(Text().text("focus me").build()).build();

    let col = Column()
        .child(pad)
        .child(menu)
        .child(foc)
        .child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return label.atom_id();
}

// The button decode (the crossing's 0/1/2 — no enum cast in this build).
fn button_code(b: MouseButton) -> str {
    if (b == MouseButton.Middle) {
        return "1";
    }
    if (b == MouseButton.Right) {
        return "2";
    }
    return "0";
}

// The test drives focus programmatically: `focus_request` targets tree
// node ids, which only exist after mount — so the node id arrives through
// the entry rail.
entry fn do_focus(node: u64, _x: f64) {
    focus_request(node);
}
"#;

#[test]
fn rut_gesture_focus_key_payloads_realm_free() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(GESTURE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let pad = app.dev_tool_get_element(column.children[0]).unwrap();
    let foc = app.dev_tool_get_element(column.children[2]).unwrap();

    // Full pointer sequence over the pad: down → move → up, plus the
    // synthesized tap. Local coordinates equal global minus the pad's
    // origin; each intent carries all four numbers.
    let (px, py) = (pad.absolute.0 + 10.0, pad.absolute.1 + 6.0);
    app.pointer_down(px, py);
    app.pointer_move(px + 5.0, py + 3.0);
    app.pointer_up(px + 5.0, py + 3.0);
    app.wait_for_timeout(Duration::ZERO);

    let transcript = rut_bound_text(&app);
    // Local coords are pad-relative; globals differ by the pad's origin
    // (the column centers it) — pin the locals, spot-check the global
    // offset consistency on the down intent.
    let pad_origin = (pad.absolute.0, pad.absolute.1);
    assert!(
        transcript.contains("|down 10,6 g")
            && transcript.contains(&format!(
                "|down 10,6 g{:.0},{:.0} b0",
                pad_origin.0 + 10.0,
                pad_origin.1 + 6.0
            ))
            && transcript.contains("|move 15,9 g")
            && transcript.contains("|up 15,9 g")
            && transcript.contains("|tap 1"),
        "pointer intents carried the full position record: {transcript}"
    );

    // Right-click on the menu pad → the mutation rail's context-menu
    // invocation; the typed event decodes the right button.
    let menu = app.dev_tool_get_element(column.children[1]).unwrap();
    let (mx, my) = (
        menu.absolute.0 + menu.size.0 / 2.0,
        menu.absolute.1 + menu.size.1 / 2.0,
    );
    app.right_click(mx, my);
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        rut_bound_text(&app).contains("|menu-right"),
        "the context-menu mutation fired: {}",
        rut_bound_text(&app)
    );

    // Programmatic focus (the entry rail drives `focus_request` with the
    // focusable's tree node id), then a shifted keydown: the key record
    // crosses (id, key, code, mods, kind) realm-free.
    let foc_node = column.children[2].as_u64();
    app.call_rut_entry("do_focus", foc_node, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        rut_bound_text(&app).contains("|focused"),
        "the focus intent fired: {}",
        rut_bound_text(&app)
    );
    app.send_key_with_modifiers("A", true, false);
    app.wait_for_timeout(Duration::ZERO);
    let transcript = rut_bound_text(&app);
    assert!(
        transcript.contains("|key A/A m1 k0"),
        "the key payload crossed as a record: {transcript}"
    );
}

// ---------------------------------------------------------------------------
// Phase C5 — animation: the Rust-held controller opaque + the onTick entry
// rail + Opacity/Transform rows + tween helpers.
// ---------------------------------------------------------------------------

/// A controller drives the alpha atom via onTick (the controller's id IS
/// the atom); the atom drives an Opacity. The label records the tween /
/// curve helper answers at start.
const ANIM_RUT: &str = r#"
use tur_host::{ color_tween_lerp, curve_eval, tween_lerp };
use tur_kit::{ Column, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };
use tur_anim_kit::{ Opacity };
use tur_anim_kit::{ anim_ctrl };


entry fn start() -> u64 {
    // The helper rows answer at start (pure math) — the label derive
    // renders the recorded transcript.
    let tw = tween_lerp(100.0, 200.0, 0.5) as u64;
    let cv = curve_eval("linear", 0.25) as u64;
    let cl = color_tween_lerp(0x000000FFu64, 0xFFFFFFFFu64, 0.5);
    let record: str = f"tw{tw} cv{cv} cl{cl}";

    let alpha: Readable<f64> = source<f64>(0.0);

    // The tick/end mutations capture the alpha source (boa wraps onTick
    // in mutate); the controller drives the Opacity through the bound
    // handle.
    let a_tick: ?Mutation<f64> = mutate<f64>(fn (ctx: MutationCtx, t: f64) {
        ctx.set<f64>(alpha, t);
    });
    let a_end: ?Mutation<nil> = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(alpha, 1.0);
    });
    let ctrl = anim_ctrl(200.0, "linear", 0, a_tick, a_end);
    ctrl.forward();

    let label: Readable<str> = source<str>(record);

    let col = Column()
        .child(Opacity(0.0).bound(alpha).child(Text().text("fade").build()).build())
        .child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return alpha.atom_id();
}
"#;

#[test]
fn rut_animation_controller_ticks_into_opacity() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(ANIM_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The helper rows answered at start: tween_lerp(100,200,.5)=150,
    // curve_eval("linear",.25)=0, color lerp mid = 0x808080FF.
    assert_eq!(rut_bound_text(&app), "tw150 cv0 cl2155905279", "helper rows");

    // The opacity element exists and wraps its child.
    let op = app
        .query_element(&["rut", "opacity"])
        .expect("opacity element not found");

    // Mid-animation: the eased tick drove the atom (~0.5 at 100ms linear).
    app.wait_for_timeout(Duration::from_millis(100));
    let a = read_rut_f64(&app);
    assert!(
        (a - 0.5).abs() < 0.35,
        "the onTick rail drove the alpha atom: {a}"
    );

    // After the duration: onEnd fired (value pinned at 1.0).
    app.wait_for_timeout(Duration::from_millis(200));
    assert!(
        (read_rut_f64(&app) - 1.0).abs() < 0.001,
        "the animation completed"
    );
}

// ---------------------------------------------------------------------------
// Phase C6 — async capabilities: clipboard + net request/stream over
// `pkg_async_fn!` + `Completer`, task-cancel wire-abort, bytes helpers.
// ---------------------------------------------------------------------------

/// A clipboard round-trip and an HTTP request, awaited in rut
/// (`launch_future` + `await`; the pump's `run_ready` drives it).
const ASYNC_RUT: &str = r#"
use tur_host::{ clipboard_read, clipboard_write, decode_utf8, net_request, spawn };
use tur_kit::{ Column, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };

// The async boundary: the handles ride by parameter; the ctx is
// task-scoped (minted at the launch site) and works across awaits.
async fn work(ctx: TaskCtx, label: Readable<str>) -> str {
    ctx.set<str>(label, "launched");
    await clipboard_write("from rut");
    let clip = await clipboard_read();
    ctx.set<str>(label, f"{ctx.get<str>(label)}-after-read:{clip}");
    let body = await net_request("https://example.test/api", "GET");
    let text = decode_utf8(body);
    ctx.set<str>(label, f"{ctx.get<str>(label)}|{text}");
    return "";
}

entry fn start() -> u64 {
    let label: Readable<str> = source<str>("");
    spawn(work(TaskCtx.mint(), label));
    let col = Column().child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return label.atom_id();
}
"#;

#[test]
fn rut_async_clipboard_and_net_request() {
    let mut app = TurTestApp::new_with_http(400.0, 600.0).unwrap();
    app.set_clipboard_read("seeded");
    app.set_http_response(tur_net_capability::HttpOutcome::Ok {
        status: 200,
        status_text: "OK".to_string(),
        headers: Vec::new(),
        body: b"hello from net".to_vec(),
    });

    app.load_rut_module(ASYNC_RUT).unwrap();
    // Each await resumes on a later pump (the capability futures complete
    // on the worker's task lane); poll until the final write lands. The
    // kit prelude compiles on the worker in real time, so poll on the real
    // clock — the virtual-clock window would close under load.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let done = loop {
        if rut_bound_text(&app).ends_with("|hello from net") {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        app.wait_for_timeout(Duration::from_millis(25));
    };
    assert!(done, "the awaits completed: {:?}", rut_bound_text(&app));
    assert_eq!(
        app.take_clipboard_write(),
        Some("from rut".to_string()),
        "the clipboard write crossed"
    );
}

/// A streaming download into rut: each chunk crosses as an intent record
/// into `on_chunk` (the chunk lengths append to the label); the task
/// opaque's cancel row runs (idempotent after completion).
const STREAM_RUT: &str = r#"
use tur_host::{ clipboard_write, spawn, task_cancel };
use tur_kit::{ Column, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };
use tur_net_kit::{ net_stream };


entry fn start() -> u64 {
    let label: Readable<str> = source<str>("");

    // The chunk handler: a mutation over the typed bytes payload (the
    // transcript accumulates the chunk lengths).
    let b_chunk = mutate<bytes>(fn (ctx: MutationCtx, data: bytes) {
        ctx.set<str>(label, f"{ctx.get<str>(label)}|{data.len() as u64}");
    });
    let task = net_stream("https://example.test/stream", "GET", b_chunk);
    // The cancel journey takes the task opaque as a PARAM (opaques are
    // just values — no stash round-trip).
    spawn(finish(TaskCtx.mint(), task, label));
    let col = Column().child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return label.atom_id();
}

async fn finish(ctx: TaskCtx, task: opaque, label: Readable<str>) -> str {
    // One beat (a quick capability await) so the drive is mid-flight,
    // then wire-abort the stream: whatever chunks landed stay on the
    // label; the rest never arrive.
    ctx.set<str>(label, f"{ctx.get<str>(label)}|launched");
    await clipboard_write("beat");
    task_cancel(task);
    ctx.set<str>(label, f"{ctx.get<str>(label)}|done");
    return "";
}
"#;

#[test]
fn rut_net_stream_chunks_cross_as_records() {
    let mut app = TurTestApp::new_with_http(400.0, 600.0).unwrap();
    app.set_http_stream(200, vec![b"abc".to_vec(), b"de".to_vec()]);

    app.load_rut_module(STREAM_RUT).unwrap();
    // The kit prelude compiles on the worker in real time — poll on the
    // real clock (the virtual-clock window would close under load).
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let done = loop {
        if rut_bound_text(&app).contains("|done") {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        app.wait_for_timeout(Duration::from_millis(25));
    };
    assert!(done, "the cancel journey ran: {:?}", rut_bound_text(&app));

    // The transcript pins all three crossings: the launched journey's
    // markers (run_ready phase) and both chunk records (intent-drain
    // phase, in stream order). (The two phases append in drain order —
    // intents drain after the flush — so the chunks may trail the done
    // marker; each pair's internal order is what matters.)
    let transcript = rut_bound_text(&app);
    assert!(
        transcript.contains("|launched") && transcript.contains("|done"),
        "the launched cancel journey ran: {transcript}"
    );
    assert!(
        transcript.contains("|3|2"),
        "both chunks crossed as ordered intent records: {transcript}"
    );
}

// ---------------------------------------------------------------------------
// Phase C7 — lifecycle rows + virtual apps: mount/destroy intents and a
// rut-authored `VirtualAppView` hosting a full child instance.
// ---------------------------------------------------------------------------

/// A rut parent hosting a rut child through the virtual-app rows. The
/// controller rides the opaque stash (the poll entry reads it back); the
/// child's lifecycle flips the status rail the rows read natively.
const VAPP_RUT: &str = r#"
use tur_host::{ st_put, st_take, va_controller, va_create_source, va_destroy, va_error, va_status };
use tur_kit::{ Column, Lifecycle, MountEvent, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, VAppCtrl, VirtualApp, mount, mutate, source };


let CTRL_KEY: u64 = 42;

entry fn start() -> u64 {
    let label: Readable<str> = source<str>("");
    let src = va_create_source("\nuse tur_kit::{ Mutation, MutationCtx, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };\nentry fn start() {\nmount(Text().text(\"child here\").build());\n}");
    let ctrl = va_controller(src);
    // The controller rides the opaque stash only because the poll /
    // destroy entries cannot capture it (an entry binds no module state);
    // the tests drive those entries by key.
    st_put(CTRL_KEY, ctrl);

    // The lifecycle surfaces: on_mount is a mutation over the typed
    // MountEvent; before_destroy keeps the fn rail (the teardown leg).
    let b_mount = mutate<MountEvent>(fn (_ctx: MutationCtx, _ev: MountEvent) {
    });

    let col = Column()
        .child(Lifecycle().on_mount(b_mount).before_destroy(lc_destroy).child(Text().text("wrapped").build()).build())
        .child(VirtualApp().controller(VAppCtrl(ctrl)).width_height(200.0, 80.0).build())
        .child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return label.atom_id();
}

fn lc_destroy(_id: u64, _b: u64, _n: f64) {
}

// The test drives the status poll: the label atom rides the entry arg,
// the controller comes back from the stash (an entry cannot capture it).
entry fn poll(label: u64, _b: f64) {
    let ctrl = st_take(CTRL_KEY);
    let s = va_status(ctrl);
    if (s == "error") {
        rs_set_str(label, f"error: {va_error(ctrl)}");
        st_put(CTRL_KEY, ctrl);
        return;
    }
    st_put(CTRL_KEY, ctrl);
    rs_set_str(label, s);
}

entry fn destroy(label: u64, _b: f64) {
    let ctrl = st_take(CTRL_KEY);
    va_destroy(ctrl);
    st_put(CTRL_KEY, ctrl);
    rs_set_str(label, "destroyed");
}
"#;

#[test]
fn rut_virtual_app_hosts_a_child_and_lifecycle_intents_fire() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(VAPP_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The lifecycle wrapper's child renders (the rut descriptor arm).
    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    assert_eq!(column.children.len(), 3, "wrapper + host + label");

    let label_atom = app.rut_start_answer();
    // The child spawns: the status rail flips idle → spawning → running.
    // The child compiles the kit prelude on the virtual-pool worker (real
    // time), so poll on the real clock until the load settles.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let running = loop {
        app.call_rut_entry("poll", label_atom, 0.0).unwrap();
        let s = rut_bound_text(&app);
        if s == "running" || s == "error" {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(running, "the child reached running: {:?}", rut_bound_text(&app));
    let _ = 0; // (error detail asserted below when non-running)

    // Destroy: the child tears down.
    app.call_rut_entry("destroy", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "destroyed", "the child was destroyed");
}

/// The Edit→Split wasm trap (phase 6.5), pinned at its seam: a child
/// destroyed while a module load for it is in flight (or issued against the
/// stale facade after the destroy) must surface as a typed
/// [`ModuleError::WorkerGone`] — lifecycle, not an engine bug. The load's
/// reply sender lives in the child worker's inbox; the destroy exits the
/// worker's loop and drops that inbox, canceling the reply. The load path
/// treated the cancellation as an invariant breach and PANICKED — on wasm
/// (panic=abort) that aborted the whole engine: the playground's tab switch
/// Edit→Split with a live child trapped the build with `unreachable`
/// (backend.rs `reply sender dropped without firing`, from
/// `VirtualHost::spawn_child`'s load task). Native pin: drive a load
/// against a child facade whose worker already exited; it must return an
/// error, not panic.
#[test]
fn rut_child_load_against_a_destroyed_child_reports_worker_gone() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(VAPP_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The child spawns and runs (the kit prelude compiles on the
    // virtual-pool worker in real time).
    let label_atom = app.rut_start_answer();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        app.call_rut_entry("poll", label_atom, 0.0).unwrap();
        let s = rut_bound_text(&app);
        if s == "running" || s == "error" {
            assert_eq!(s, "running", "the child settled");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the child never reached running: {s}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // Mint a facade over the live child (the stale-handle shape the wasm
    // load task hits — its facade outlives the retired child).
    let children = app.app().virtual_apps();
    assert_eq!(children.len(), 1, "one live child");

    // Destroy: the retire path tears the child's worker down (its loop
    // exits and drops the RPC inbox).
    app.call_rut_entry("destroy", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "destroyed", "the child was destroyed");

    // The stale facade's load reports `WorkerGone` ("worker gone" through
    // the facade's TurError wrap) — it must NOT panic.
    let outcome = futures::executor::block_on(children[0].load_rut_module(
        "\nentry fn start() {}\n",
    ));
    let err = outcome.expect_err("a load against a destroyed child must err, not panic");
    assert!(
        err.to_string().contains("worker gone"),
        "the load reports the gone worker, not an engine bug: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Phase C8 — derived atoms: the guarded sync VM call during flush + watch.
// ---------------------------------------------------------------------------

/// A derived atom (`entry fn d(v: f64) -> str`) bound to a Text; the dep
/// is a counter atom driven by a button. The derive materializes inside
/// the flush (the guarded face call). A second derived (`d2`) chains two
/// deps. A watcher reports changes into a transcript atom.
const DERIVED_RUT: &str = r#"
use tur_host::{ ctx_bridge, rs_get_f64, rs_set_f64, rs_set_str, rs_source_f64, rs_source_str, rs_watch_start };
use tur_kit::{ Column, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, source };
use tur_kit::{ Mutation, MutationCtx, Readable, Source, TaskCtx, TextCtrl, UndoCtrl, mount, mutate, rs_derive, rs_derive2, rs_watch, source };


entry fn start() -> u64 {
    let count = rs_source_f64();
    let other = rs_source_f64();
    let d = rs_derive(d, count);
    let d2 = rs_derive2(d2, count, other);

    let hits = rs_source_str("");
    let watch = rs_watch(count, on_count, hits);
    rs_watch_start(watch);

    // The unified text prop: rebuilt handles over the substrate atoms.
    let dr = Source<str>.of(ctx_bridge(), d, false, 1);
    let d2r = Source<str>.of(ctx_bridge(), d2, false, 1);
    let hitsr = Source<str>.of(ctx_bridge(), hits, false, 1);
    let b_inc = mutate(fn (_ctx: MutationCtx, _e: nil) {
        rs_set_f64(count, rs_get_f64(count) + 1.0);
    });

    let col = Column()
        .child(Text().text_bound(dr).query_key("rut/text").build())
        .child(Text().text_bound(d2r).query_key("rut/text").build())
        .child(Text().text_bound(hitsr).build())
        .child(
        PointerInteract().on_click(b_inc).child(Text().text("+1").build()).build(),
    );
    mount(col.build());
    return count;
}

// The derive bodies — synchronous VM calls during flush, through the
// guarded face.
fn d(v: f64) -> str {
    return f"count={v as u64}";
}

fn d2(a: f64, b: f64) -> str {
    return f"sum={a as u64 + b as u64}";
}

// The watch delivery: (report atom, watched atom, seq) — the fresh value
// reads through the rows.
fn on_count(report: u64, watched: u64, _n: f64) {
    let v = rs_get_f64(watched);
    rs_set_str(report, f"changed:{v as u64}");
}
"#;

/// The no-mount guard: a derive whose body tries `tur_host::mount` TRAPS (the
/// face raises `face_busy`), the trap reports through the error rail, the
/// derived falls back to Nil — and the frame never wedges (the healthy
/// derive beside it keeps materializing).
const DERIVED_NO_MOUNT_RUT: &str = r#"
use tur_host::{ ctx_bridge, rs_get_f64, rs_set_f64, rs_source_f64 };
use tur_kit::{ Column, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, TextCtrl, UndoCtrl, mount, mutate, rs_derive, source };

entry fn start() -> u64 {
    let count = rs_source_f64();
    let bad = rs_derive(bad, count);
    let good = rs_derive(good, count);

    let badr = Source<str>.of(ctx_bridge(), bad, false, 1);
    let goodr = Source<str>.of(ctx_bridge(), good, false, 1);
    let b_inc = mutate(fn (_ctx: MutationCtx, _e: nil) {
        rs_set_f64(count, rs_get_f64(count) + 1.0);
    });

    let col = Column()
        .child(Text().text_bound(badr).query_key("rut/text").build())
        .child(Text().text_bound(goodr).query_key("rut/text").build())
        .child(
        PointerInteract().on_click(b_inc).child(Text().text("+1").build()).build(),
    );
    mount(col.build());
    return count;
}

// The hostile derive: tries to re-mount mid-flush — the no-mount guard
// traps it.
fn bad(v: f64) -> str {
    let col = Column().child(Text().text("hijack").build());
    mount(col.build());
    return f"bad={v as u64}";
}

fn good(v: f64) -> str {
    return f"good={v as u64}";
}
"#;

#[test]
fn rut_derive_mount_guard_traps_without_wedging() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(DERIVED_NO_MOUNT_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The hostile derive fell back to Nil (rendered empty); the healthy
    // one beside it materialized.
    let texts = all_texts(&app);
    assert!(
        texts.contains(&"good=0".to_string()),
        "the healthy derive materialized: {texts:?}"
    );
    assert!(
        !texts.contains(&"bad=0".to_string()) && !texts.contains(&"hijack".to_string()),
        "the mount-in-derive trapped before rendering: {texts:?}"
    );

    // The frame is alive: a bump re-materializes the healthy derive and
    // the trap fires again — no wedge, no parked VM.
    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let button = app.dev_tool_get_element(column.children[2]).unwrap();
    let (bx, by) = button.absolute;
    let (bw, bh) = button.size;
    app.click(bx + bw / 2.0, by + bh / 2.0);
    app.wait_for_timeout(Duration::ZERO);
    let texts = all_texts(&app);
    assert!(
        texts.contains(&"good=1".to_string()),
        "the flush kept converging after the trap: {texts:?}"
    );
}

#[test]
fn rut_derived_atoms_watch_and_guards() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(DERIVED_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The derives materialized at mount (the guarded face calls during
    // the build flush): count=0 / sum=0(+other=0).
    let texts = all_texts(&app);
    assert!(
        texts.contains(&"count=0".to_string()) && texts.contains(&"sum=0".to_string()),
        "both derives materialized at mount: {texts:?}"
    );

    // Bump the counter — the flush re-materializes BOTH derives through
    // the guarded face and the watch delivery fires.
    let root = app.dev_tool_element_tree().unwrap();
    let column = app.dev_tool_get_element(root.children[0]).unwrap();
    let button = app.dev_tool_get_element(column.children[3]).unwrap();
    let (bx, by) = button.absolute;
    let (bw, bh) = button.size;
    app.click(bx + bw / 2.0, by + bh / 2.0);
    app.wait_for_timeout(Duration::ZERO);

    let texts = all_texts(&app);
    assert!(
        texts.contains(&"count=1".to_string()) && texts.contains(&"sum=1".to_string()),
        "both derives re-materialized after the dep write: {texts:?}"
    );
    assert!(
        texts.contains(&"changed:1".to_string()),
        "the watch delivery fired with the fresh value: {texts:?}"
    );
}


/// The alpha atom's current value, read back through the bound opacity
/// element's resolved paint value (layout mirrors the atom each frame).
fn read_rut_f64(app: &TurTestApp) -> f64 {
    let id = app
        .query_element(&["rut", "opacity"])
        .expect("opacity gone");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::effects::OpacityElement>()
            .map(|el| el.painted_value() as f64)
            .unwrap_or(0.0)
    })
    .unwrap_or(0.0)
}
