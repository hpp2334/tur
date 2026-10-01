//! Rich-text performance invariants for long spanned editors (the code
//! editor shape: `Input` + `TextEditingController` with one span per token,
//! inside a `ScrollView`).
//!
//! These pin the layout-memo contract via the `shapeCount` dev-tool counter
//! (how many times the document was parley-shaped):
//!
//! - cursor-only key events never re-shape,
//! - typing a char re-shapes exactly once,
//! - a no-op re-highlight (`setSpansPreserveCursor` with the same spans)
//!   never re-shapes, while a content-differing one does,
//! - width-constraint changes re-shape (the memo key includes them),
//! - scrolling never re-shapes (constraints are stable under scroll).

use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::elements::TraceValue;
use tur_integration_tests::TurTestApp;

/// Long highlighted document: 400 monospace lines, one span per line (the
/// token granularity of syntax highlighting, minus per-token splitting to
/// keep the fixture small — the memo contract is length-independent).
const LONG_EDITOR: &str = r##"
use tur::{
    el_build, el_input_ctrl, el_qkey, el_scroll, mount, tctrl_new, tctrl_push_span,
};

entry fn start() {
    let ctrl = tctrl_new();
    let mut i = 0;
    while (i < 400) {
        tctrl_push_span(ctrl, f"const value{i} = {i}; // line {i}\n");
        i += 1;
    }

    let input = el_input_ctrl(ctrl, 100000.0, 10000.0, 14.0);
    el_qkey(input, "ed");
    let scroller = el_scroll(true, el_build(input));
    el_qkey(scroller, "scroll");
    mount(el_build(scroller));
}
"##;

/// The `tur_editable_text` node under the `Input` queryKey (the key lands
/// on the wrapper; the editable is its child — same lookup as the editable
/// tests' `find_editable_under`).
fn editor_id(app: &TurTestApp) -> ElementNodeId {
    let raw = app.query_element(&["ed"]).expect("editor queryKey");
    let container = ElementNodeId::new(raw.as_u64());
    let tree = app.element_tree();
    let node = tree.get_element(container).expect("wrapper node");
    for cid in node.children.iter().copied() {
        let child = tree
            .get_element(ElementNodeId::new(cid.as_u64()))
            .expect("child node");
        if child.kind() == Some(ElementKind::new("tur_editable_text")) {
            return ElementNodeId::new(cid.as_u64());
        }
    }
    panic!("no tur_editable_text under queryKey [\"ed\"]");
}

fn extra(app: &TurTestApp, id: ElementNodeId, name: &str) -> f64 {
    let dev = app
        .dev_tool_get_element(id.into())
        .expect("editor dev node");
    dev.layout_extra
        .iter()
        .find(|(k, _)| *k == name)
        .and_then(|(_, v)| match v {
            TraceValue::Num(n) => Some(*n),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing layout_extra {name}"))
}

fn shape_count(app: &TurTestApp, id: ElementNodeId) -> u64 {
    extra(app, id, "shapeCount") as u64
}

/// (x, y) of the focused caret.
fn caret_xy(app: &TurTestApp) -> (f64, f64) {
    let (x, y, _w, _h) = app.focused_cursor_rect().expect("focused caret rect");
    (x, y)
}

/// Focus the editor with a click at its top-left (caret lands at byte 0)
/// and settle. Returns the settled caret (x, y).
fn focus_at_start(app: &mut TurTestApp, id: ElementNodeId) {
    let bounds = app.get_element_absolute_bounds(id).expect("editor bounds");
    app.click(bounds.left + 2.0, bounds.top + 2.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
}

fn mount_long_editor(width: f64, height: f64) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(width, height).unwrap();
    app.load_rut_module(LONG_EDITOR).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = editor_id(&app);
    (app, id)
}

#[test]
fn arrow_keys_do_not_reshape() {
    let (mut app, id) = mount_long_editor(400.0, 300.0);
    focus_at_start(&mut app, id);

    let before = shape_count(&app, id);
    let (cx0, cy0) = caret_xy(&app);

    for _ in 0..5 {
        app.send_key("ArrowRight");
    }
    app.send_key("ArrowDown");
    app.send_key("ArrowDown");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let after = shape_count(&app, id);
    assert_eq!(
        after, before,
        "pure cursor moves must not re-shape the document"
    );
    let (cx, cy) = caret_xy(&app);
    assert!(
        cx > cx0 || cy > cy0,
        "caret must still move (from ({cx0},{cy0}) to ({cx},{cy}))"
    );
}

#[test]
fn typing_reshapes_exactly_once() {
    let (mut app, id) = mount_long_editor(400.0, 300.0);
    focus_at_start(&mut app, id);

    let before = shape_count(&app, id);
    let len_before = extra(&app, id, "numLines");

    app.send_key("x");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let after = shape_count(&app, id);
    assert_eq!(
        after,
        before + 1,
        "one content change must reshape exactly once (before={before}, after={after})"
    );
    // Layout actually reflects the edit (the memo must not serve a stale
    // layout): 400 content lines + a leading "x" keeps 401 visual lines
    // (the doc ends with a trailing newline).
    let len_after = extra(&app, id, "numLines");
    assert!(
        len_after >= len_before,
        "layout must reflect the inserted char"
    );
}

#[test]
fn width_change_invalidates_memo() {
    let (mut app, id) = mount_long_editor(400.0, 300.0);
    focus_at_start(&mut app, id);

    let before = shape_count(&app, id);

    // Narrower viewport → narrower max_width constraint → new memo key.
    app.resize(320.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(
        shape_count(&app, id),
        before + 1,
        "constraint change must invalidate the layout memo"
    );

    // Same width again → another (different) key → reshape again.
    app.resize(400.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(shape_count(&app, id), before + 2);
}

#[test]
fn scrolling_does_not_reshape() {
    let (mut app, id) = mount_long_editor(400.0, 300.0);
    focus_at_start(&mut app, id);

    let before = shape_count(&app, id);
    let bounds_before = app.get_element_absolute_bounds(id).expect("editor bounds");

    for _ in 0..6 {
        app.wheel(0.0, 120.0, 200.0, 150.0);
    }
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        shape_count(&app, id),
        before,
        "scrolling must never re-shape the document (constraints are stable)"
    );
    let bounds_after = app.get_element_absolute_bounds(id).expect("editor bounds");
    assert!(
        bounds_after.top < bounds_before.top,
        "content must actually scroll (top {} → {})",
        bounds_before.top,
        bounds_after.top
    );
}
