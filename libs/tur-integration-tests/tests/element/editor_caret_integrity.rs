//! Editor caret integrity — the playground editor's live-reported bugs,
//! round two (real-browser diagnosed).
//!
//! The served playground reported (operator-verified with real input):
//!
//! 1. Clicks stop placing the caret where the user clicked: after deep
//!    scrolling / layout-mode switches, the drawn caret, the insert point,
//!    and the controller cursor disagree; clicks on thin strips (rows
//!    without ink — e.g. empty source lines) teleport the caret to the
//!    END of the document.
//! 2. Drag-selecting near the bottom edge scrolls one row per edge
//!    crossing, and the user sees the content jump even though the
//!    dragged content was already in view.
//!
//! The fixture mirrors the playground's editor slot exactly: ONE multiline
//! `Input` behind a layout-mode `Switch` whose branches size the SAME pane
//! (zero-box "view", 200px "split", 400px "edit" — the mode switch remounts
//! the editor while the controller, its text, cursor and selection, are
//! shared and survive). The document carries EMPTY lines (rows without
//! glyph stops) at `i % 7 == 3`.

use std::time::Duration;

use tur_engine::builtin_plugins::text::elements::EditableTextElement;
use tur_engine::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use tur_integration_tests::TurTestApp;

const ZERO: Duration = Duration::ZERO;

const EDITOR_RUT: &str = r#"
use tur_kit::{ Clip, Container, Input, Source, Switch, TextCtrl, UndoCtrl, entry_ctx, mount, source, text_ctrl, undo_ctrl };

struct EditorCx {
    ctrl: TextCtrl,
    undo: UndoCtrl,
    mode: Source<str>,
}

fn long_doc() -> str {
    let mut text = "";
    let mut i = 0;
    while (i < 80) {
        if (i % 7 == 3) {
            text = f"{text}\n";
        } else {
            text = f"{text}line {i}\n";
        }
        i = i + 1;
    }
    return text;
}

fn start() -> EditorCx {
    let ctrl = text_ctrl();
    ctrl.set_text(long_doc());
    let undo = undo_ctrl();
    let mode = source<str>("split");
    let editor = Input().controller(ctrl).undo(undo)
        .multiline(true)
        .font_family("monospace")
        .font_size(13.0)
        .width_height(0.0, 120.0)
        .query_key("ed")
        .build();
    let pane = Container().child(editor).build();
    let slot = Switch().value(mode)
        .cases("view", Container().width(0.0).height(0.0).clip(Clip.HardEdge).child(pane).build())
        .cases("split", Container().width(200.0).child(pane).build())
        .fallback(Container().width(400.0).child(pane).build())
        .query_key("slot")
        .build();
    mount(slot);
    return EditorCx { ctrl: ctrl, undo: undo, mode: mode };
}

entry fn entry_start() -> opaque {
    let cx = start();
    return opaque(cx);
}

fn editor_cx(cx: opaque) -> EditorCx {
    let c = opaque.downcast<EditorCx>(cx);
    if (c == nil) {
        panic("editor fixture: cx is not an EditorCx");
    }
    return c;
}

entry fn set_mode(cx: opaque, code: f64) {
    let c = editor_cx(cx);
    let mut name = "split";
    if (code == 1.0) { name = "edit"; }
    if (code == 2.0) { name = "view"; }
    entry_ctx().set<str>(c.mode, name);
}
"#;

/// The Rust mirror of `long_doc()` — the byte-exact same string.
fn mirror_doc() -> String {
    let mut text = String::new();
    for i in 0..80 {
        if i % 7 == 3 {
            text.push('\n');
        } else {
            text.push_str(&format!("line {i}\n"));
        }
    }
    text
}

/// Byte offset of the start of every line (line `k` starts at
/// `line_starts[k]`; an empty line has the same start as the next line).
fn line_starts(doc: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, b) in doc.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn setup(source: &str) -> (TurTestApp, ElementNodeId, u64) {
    let mut app = TurTestApp::new(500.0, 300.0).unwrap();
    app.load_rut_module(source).unwrap();
    let cx = app.call_rut_entry_opaque("entry_start").unwrap();
    app.wait_for_timeout(ZERO);
    let ed = find_editable(&app);
    (app, ed, cx)
}

/// Locate the live `EditableTextElement` anywhere in the tree (the Switch
/// branch remounts it on every mode switch, so the id is not stable).
fn find_editable(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root_element().expect("root element");
    let mut stack: Vec<u64> = root.children.iter().map(|c| c.as_u64()).collect();
    let mut found = None;
    while let Some(id) = stack.pop() {
        if let Some(snap) = tree.get_element(ElementNodeId::new(id)) {
            stack.extend(snap.children.iter().map(|c| c.as_u64()));
            let eid = ElementNodeId::new(id);
            let is_editable = app
                .with_element(eid, |e| e.cast::<EditableTextElement>().is_some())
                .unwrap();
            if is_editable {
                found = Some(eid);
                break;
            }
        } else if let Some(frag) = tree.fragments.get(&FragmentNodeId::new(id)) {
            stack.extend(frag.children.iter().map(|c| c.as_u64()));
        }
    }
    found.expect("an EditableTextElement in the tree")
}

fn scroll_of(app: &TurTestApp, ed: ElementNodeId) -> f64 {
    app.with_element(ed, |e| e.cast::<EditableTextElement>().unwrap().scroll_y())
        .unwrap()
}

fn text_of(app: &TurTestApp, ed: ElementNodeId) -> String {
    app.with_element(ed, |e| e.cast::<EditableTextElement>().unwrap().text())
        .unwrap()
}

fn cursor_of(app: &TurTestApp, ed: ElementNodeId) -> usize {
    app.with_element(ed, |e| {
        e.cast::<EditableTextElement>().unwrap().cursor_position()
    })
    .unwrap()
}

fn selection_of(app: &TurTestApp, ed: ElementNodeId) -> (usize, usize) {
    app.with_element(ed, |e| e.cast::<EditableTextElement>().unwrap().selection())
        .unwrap()
}

/// Absolute (x, y, w, h) of the editable's box on screen: a DFS from the
/// root accumulating each element's layout offset (fragments contribute
/// none — their children splice into the enclosing flex).
fn editor_box(app: &TurTestApp, ed: ElementNodeId) -> (f64, f64, f64, f64) {
    let tree = app.element_tree();
    let root = tree.root_element().expect("root element");
    let mut found = None;
    fn walk(
        tree: &tur_engine::core::elements::NodeTreeSnapshot,
        id: u64,
        x: f64,
        y: f64,
        want: ElementNodeId,
        found: &mut Option<(f64, f64, f64, f64)>,
    ) {
        if found.is_some() {
            return;
        }
        if let Some(snap) = tree.get_element(ElementNodeId::new(id)) {
            let (x, y) = (
                x + snap.computed_layout.offset.x,
                y + snap.computed_layout.offset.y,
            );
            if snap.id == want {
                *found = Some((
                    x,
                    y,
                    snap.computed_layout.size.width,
                    snap.computed_layout.size.height,
                ));
                return;
            }
            for c in &snap.children {
                walk(tree, c.as_u64(), x, y, want, found);
            }
        } else if let Some(frag) = tree.fragments.get(&FragmentNodeId::new(id)) {
            for c in &frag.children {
                walk(tree, c.as_u64(), x, y, want, found);
            }
        }
    }
    let (rx, ry) = (root.computed_layout.offset.x, root.computed_layout.offset.y);
    for c in &root.children {
        walk(&tree, c.as_u64(), rx, ry, ed, &mut found);
    }
    found.expect("the editable's accumulated box")
}

/// Calibrate the monospace cell + line pitch from the caret rect: returns
/// `(x0, y0, cw, line_h)` where `(x0, y0)` is the caret position at byte 0
/// (the text origin on screen) and `cw` / `line_h` are the per-char /
/// per-line advances.
fn calibrate(app: &mut TurTestApp) -> (f64, f64, f64, f64) {
    // Click near the top-left of the editor (line 0), then measure.
    let ed = find_editable(app);
    let (bx, by, bw, _) = editor_box(app, ed);
    assert!(bw > 50.0, "the editor has a real width: {bw}");
    app.click(bx + 2.5, by + 4.0);
    app.wait_for_timeout(ZERO);
    // Force the caret to byte 0 (the click may land on byte 1 depending on
    // internal padding) so x0 is the byte-0 caret x.
    for _ in 0..12 {
        app.send_key("ArrowLeft");
    }
    app.wait_for_timeout(ZERO);
    let (x0, y0, _, _) = app.focused_cursor_rect().expect("caret rect after click");
    app.send_key("ArrowRight");
    app.wait_for_timeout(ZERO);
    let (x1, _, _, _) = app.focused_cursor_rect().expect("caret rect after arrow");
    app.send_key("ArrowDown");
    app.wait_for_timeout(ZERO);
    let (_, y1, _, _) = app
        .focused_cursor_rect()
        .expect("caret rect after arrow down");
    let cw = x1 - x0;
    let line_h = y1 - y0;
    assert!(
        cw > 3.0 && line_h > 5.0,
        "calibration: cw={cw} line_h={line_h}"
    );
    (x0, y0, cw, line_h)
}

/// Screen y of content line `k` at scroll offset `s`.
fn line_y(y0: f64, line_h: f64, k: usize, s: f64) -> f64 {
    y0 + k as f64 * line_h - s
}

/// Wheel straight over the editor's center.
fn wheel_over(app: &mut TurTestApp, ed: ElementNodeId, delta_y: f64) {
    let (bx, by, bw, bh) = editor_box(app, ed);
    app.wheel(0.0, delta_y, bx + bw * 0.5, by + bh * 0.5);
    app.wait_for_timeout(ZERO);
}

/// Click content line `k`, column `c` (cell centers), while scrolled by `s`.
fn click_line(app: &mut TurTestApp, cal: (f64, f64, f64, f64), k: usize, c: usize, s: f64) {
    let (x0, y0, cw, line_h) = cal;
    let x = x0 + (c as f64 + 0.5) * cw;
    let y = line_y(y0, line_h, k, s) + 0.5 * line_h;
    app.click(x, y);
    app.wait_for_timeout(ZERO);
}

// Finding 1: after deep scrolling, clicks (including on rows WITHOUT ink —
// empty source lines) must land on the clicked row and never teleport to
// the end of the document.
#[test]
fn deep_scroll_clicks_land_where_clicked() {
    let (mut app, ed, _cx) = setup(EDITOR_RUT);
    let doc = mirror_doc();
    let starts = line_starts(&doc);
    let cal = calibrate(&mut app);
    let (_, _, _, line_h) = cal;

    // Scroll deep: line 25 at the top of the viewport.
    let s = 25.0 * line_h;
    wheel_over(&mut app, ed, s);
    let s = scroll_of(&app, ed);
    assert!((s - 25.0 * line_h).abs() < 1.0, "scrolled deep: s={s}");

    // Normal rows around the fold (line 32 is the last fully visible at
    // this scroll — the 120px viewport shows ~7.9 lines).
    for &(k, c) in &[(26usize, 2usize), (27, 5), (30, 0), (32, 4)] {
        click_line(&mut app, cal, k, c, s);
        let cursor = cursor_of(&app, ed);
        assert_eq!(
            cursor,
            starts[k] + c,
            "click on line {k} col {c} while scrolled must land there"
        );
        let (_, cy, _, ch) = app.focused_cursor_rect().unwrap();
        let want_top = line_y(cal.1, line_h, k, s);
        assert!(
            cy >= want_top - 2.0 && cy + ch <= want_top + line_h + 2.0,
            "caret for line {k} must draw on the clicked row: caret y={cy} h={ch}, row y={want_top}"
        );
    }

    // The empty lines (i % 7 == 3): 31 is visible in this window.
    click_line(&mut app, cal, 31, 2, s);
    let cursor = cursor_of(&app, ed);
    assert_eq!(
        cursor,
        starts[31],
        "clicking the empty line 31 must keep the caret on that line (got {cursor}, doc len {})",
        doc.len()
    );
    let (_, cy, _, ch) = app.focused_cursor_rect().unwrap();
    let want_top = line_y(cal.1, line_h, 31, s);
    assert!(
        cy >= want_top - 2.0 && cy + ch <= want_top + line_h + 2.0,
        "caret for the empty line must draw on its row: caret y={cy}, row y={want_top}"
    );
}

// Finding 1's split-brain: after the playground's layout-mode switch (the
// Switch remounts the editor around the SHARED controller), a click must
// still move the caret AND the next keystroke must insert at the clicked
// position — the drawn caret, the controller cursor, and the insert point
// must agree.
#[test]
fn mode_switch_click_and_type_agree() {
    let (mut app, ed, cx) = setup(EDITOR_RUT);
    let doc = mirror_doc();
    let starts = line_starts(&doc);

    // Split mode: click line 6 col 3 (visible, a normal row).
    let cal = calibrate(&mut app);
    click_line(&mut app, cal, 6, 3, 0.0);
    assert_eq!(cursor_of(&app, ed), starts[6] + 3);

    // The playground journey: split -> view -> edit (each remounts the
    // editor; the controller survives).
    app.call_rut_entry_cx_f64("set_mode", cx, 2.0).unwrap();
    app.wait_for_timeout(ZERO);
    app.call_rut_entry_cx_f64("set_mode", cx, 1.0).unwrap();
    app.wait_for_timeout(ZERO);

    let ed2 = find_editable(&app);
    assert_ne!(ed2, ed, "the mode switch remounts the editor");

    // Recalibrate in the new (400px wide) editor and click line 5 col 4.
    let cal = calibrate(&mut app);
    click_line(&mut app, cal, 5, 4, 0.0);
    let want = starts[5] + 4;
    assert_eq!(cursor_of(&app, ed2), want, "click after the mode switch");

    // The keystroke must insert exactly at the clicked byte.
    app.send_key("x");
    app.wait_for_timeout(ZERO);

    let text = text_of(&app, ed2);
    let mut expected = doc.clone();
    expected.insert(want, 'x');
    assert_eq!(
        text, expected,
        "the keystroke must insert at the clicked byte (split-brain caret)"
    );
    assert_eq!(cursor_of(&app, ed2), want + 1);
}

// Finding 2: dragging across rows (including empty ones) selects exactly
// the dragged span and never disturbs the scroll while everything is in
// view.
#[test]
fn drag_over_empty_lines_selects_the_dragged_span() {
    let (mut app, ed, _cx) = setup(EDITOR_RUT);
    let doc = mirror_doc();
    let starts = line_starts(&doc);
    let cal = calibrate(&mut app);
    let (x0, y0, cw, line_h) = cal;

    let s = 28.0 * line_h;
    wheel_over(&mut app, ed, s);
    let s = scroll_of(&app, ed);

    // Drag line 30 col 1 -> line 33 col 4 (crossing empty line 31).
    let (x1, y1) = (x0 + 1.5 * cw, line_y(y0, line_h, 30, s) + 0.5 * line_h);
    let (x2, y2) = (x0 + 4.5 * cw, line_y(y0, line_h, 33, s) + 0.5 * line_h);
    app.pointer_down(x1, y1);
    app.wait_for_timeout(ZERO);
    app.pointer_move(x2, y2);
    app.wait_for_timeout(ZERO);
    app.pointer_up(x2, y2);
    app.wait_for_timeout(ZERO);

    let (anchor, end) = selection_of(&app, ed);
    assert_eq!(
        (anchor, end),
        (starts[30] + 1, starts[33] + 4),
        "the drag must select exactly the dragged span"
    );
    let after = scroll_of(&app, ed);
    assert!(
        (after - s).abs() < 0.001,
        "an all-visible drag must not scroll: {s} -> {after}"
    );
}

// Finding 2's bottom edge: dragging the selection past the editor's bottom
// edge must auto-scroll FORWARD (reveal the content below the fold) and
// must never scroll BACKWARD toward the top while the user drags down.
#[test]
fn bottom_edge_drag_scrolls_forward_never_backward() {
    let (mut app, ed, _cx) = setup(EDITOR_RUT);
    let cal = calibrate(&mut app);
    let (x0, y0, cw, line_h) = cal;

    let s = 30.0 * line_h;
    wheel_over(&mut app, ed, s);
    let s = scroll_of(&app, ed);

    // Press on the last visible line, then cross below the bottom edge.
    let last_visible = ((s + 120.0 - y0) / line_h).floor() as usize - 1;
    let x = x0 + 2.5 * cw;
    let y_press = line_y(y0, line_h, last_visible, s) + 0.5 * line_h;
    app.pointer_down(x, y_press);
    app.wait_for_timeout(ZERO);

    let mut prev = scroll_of(&app, ed);
    for dy in [6.0, 20.0, 40.0] {
        app.pointer_move(x, y0 + 120.0 + dy);
        app.wait_for_timeout(ZERO);
        let now = scroll_of(&app, ed);
        assert!(
            now >= prev - 0.001,
            "dragging further past the bottom edge must never scroll backward: {prev} -> {now}"
        );
        prev = now;
    }
    assert!(
        prev > s,
        "the bottom-edge auto-scroll must reveal content below the fold: {s} -> {prev}"
    );

    app.pointer_up(x, y0 + 160.0);
    app.wait_for_timeout(ZERO);
}

// The top edge mirror: dragging the selection above the editor's top edge
// must auto-scroll gently BACKWARD (a row or two — never a snap to the
// document top, never a jump past the selection).
#[test]
fn top_edge_drag_scrolls_back_gently() {
    let (mut app, ed, _cx) = setup(EDITOR_RUT);
    let cal = calibrate(&mut app);
    let (x0, y0, cw, line_h) = cal;

    let s = 30.0 * line_h;
    wheel_over(&mut app, ed, s);
    let s = scroll_of(&app, ed);

    let first_visible = ((s - y0) / line_h).ceil() as usize;
    let x = x0 + 2.5 * cw;
    let y_press = line_y(y0, line_h, first_visible, s) + 0.5 * line_h;
    app.pointer_down(x, y_press);
    app.wait_for_timeout(ZERO);

    app.pointer_move(x, y0 - 8.0);
    app.wait_for_timeout(ZERO);
    let after = scroll_of(&app, ed);
    assert!(
        after < s,
        "dragging above the top edge must auto-scroll back: {s} -> {after}"
    );
    assert!(
        after >= s - 2.0 * line_h,
        "the top-edge auto-scroll must be gentle (a row or two), not a snap: {s} -> {after}"
    );

    app.pointer_up(x, y0 - 8.0);
    app.wait_for_timeout(ZERO);
}

// The wheel must keep working through the playground's slot shape and
// across mode switches (the served build's real wheel was reported dead).
#[test]
fn wheel_scrolls_through_the_slot_shape_and_mode_switches() {
    let (mut app, ed, cx) = setup(EDITOR_RUT);
    let cal = calibrate(&mut app);
    let (_, _, _, line_h) = cal;

    wheel_over(&mut app, ed, 3.0 * line_h);
    assert!(
        (scroll_of(&app, ed) - 3.0 * line_h).abs() < 1.0,
        "wheel scrolls the slotted editor"
    );

    // Mode switch (remount) — the fresh editor must wheel immediately.
    app.call_rut_entry_cx_f64("set_mode", cx, 1.0).unwrap();
    app.wait_for_timeout(ZERO);
    let ed2 = find_editable(&app);
    assert_eq!(scroll_of(&app, ed2), 0.0, "a fresh editor opens at the top");
    wheel_over(&mut app, ed2, 2.0 * line_h);
    assert!(
        (scroll_of(&app, ed2) - 2.0 * line_h).abs() < 1.0,
        "wheel scrolls immediately after the mode switch"
    );
}
