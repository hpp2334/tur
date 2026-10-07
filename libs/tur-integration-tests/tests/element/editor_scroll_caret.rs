//! Editor scroll/caret/drag — the playground editor's live-reported bugs.
//!
//! The multiline `Input` (the playground text editor) is its own scroller.
//! Three user-visible breakages were reported against the served build:
//!
//! 1. "Click to change text cursor not work in some cases — scroll down the
//!    editor, then drag text selection."
//! 2. "The text editor can not scroll down sometimes."
//!
//! Root cause (S1): the multiline scroll refresh reset `scroll_y` on ANY
//! controller revision bump — and typing / undo / re-highlight all bump the
//! revision. Every keystroke yanked a scrolled editor back to the top (bug
//! 2: the first wheel after an edit was eaten by the same flush), and a
//! mid-interaction reset desynced the caret math the user was aiming with
//! (bug 1). The reset must fire ONLY on a true external replacement (the
//! programmatic `tctrl_set_text` rail — the playground's case switch), never
//! on typing, undo, or span writes.
//!
//! These tests pin the rails: typing keeps the scroll position; the
//! edit→wheel journey accumulates; undo keeps the scroll; the case-switch
//! replacement resets to the top (and the first wheel after it works);
//! element recreation (a module reload) leaves the fresh editor wheelable;
//! and after scrolling, a drag selects the right byte range and a click
//! places the caret on the clicked line.

use std::time::Duration;

use tur_engine::builtin_plugins::text::elements::EditableTextElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

const ZERO: Duration = Duration::ZERO;

/// 40 short monospace lines — the editor overflows its 60px viewport by
/// hundreds of pixels. Every line is exactly 7 bytes ("line {i}\n"), so line
/// `i` starts at byte `7 * i`.
const EDITOR_RUT: &str = r#"
use tur::{ mount, st_put, st_take, tctrl_new, tctrl_set_text, undo_new };
use tur_kit::{ Input };

let K_CTRL: u64 = 1;

fn long_doc() -> str {
    let mut text = "";
    let mut i = 0;
    while (i < 40) {
        text = f"{text}line {i}\n";
        i = i + 1;
    }
    return text;
}

entry fn start() -> u64 {
    let ctrl = tctrl_new();
    tctrl_set_text(ctrl, long_doc());
    st_put(K_CTRL, ctrl);
    let input = Input()
        .controller(ctrl)
        .undo(undo_new())
        .multiline(true)
        .font_family("monospace")
        .width_height(200.0, 60.0)
        .query_key("ed")
        .build();
    mount(input);
    return 0;
}

// The case-switch rail: the playground loads a new case source into the
// SAME editor via `tctrl_set_text` (a true external replacement — the view
// must reopen at the top).
entry fn load_case(_a: u64, _b: f64) {
    let ctrl = st_take(K_CTRL);
    tctrl_set_text(ctrl, long_doc());
    st_put(K_CTRL, ctrl);
}
"#;

/// Mount the editor fixture and locate the `EditableTextElement` (the
/// `Input` view is a Container wrapper whose only child is the editable).
fn setup(source: &str) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(source).unwrap();
    app.wait_for_timeout(ZERO);
    let ed = find_editable(&app);
    (app, ed)
}

/// Walk root → Input container → editable (fresh ids after any reload).
fn find_editable(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root_snap = tree.root_element().unwrap();
    assert_eq!(
        root_snap.children.len(),
        1,
        "root hosts the Input container"
    );
    let container = ElementNodeId::new(root_snap.children[0].as_u64());
    let container_snap = tree.get_element(container).unwrap();
    assert_eq!(
        container_snap.children.len(),
        1,
        "the Input container wraps one editable"
    );
    let ed = ElementNodeId::new(container_snap.children[0].as_u64());
    app.with_element(ed, |e| {
        assert!(
            e.cast::<EditableTextElement>().is_some(),
            "child is the editable"
        );
    })
    .unwrap();
    ed
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

/// S1: typing must NOT snap a scrolled editor back to the top. The caret
/// sits on the visible (scrolled-to) line, so caret-follow reveal is a
/// no-op — any scroll change is the revision-bump reset bug.
#[test]
fn typing_keeps_the_scroll_position() {
    let (mut app, ed) = setup(EDITOR_RUT);

    // Focus, scroll deep, then put the caret ON the visible line.
    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);
    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);

    let before = scroll_of(&app, ed);
    assert!(
        before > 100.0,
        "fixture precondition: scrolled, got {before}"
    );

    app.send_key("x");
    app.wait_for_timeout(ZERO);

    let after = scroll_of(&app, ed);
    assert!(
        (after - before).abs() < 0.001,
        "typing snapped the scroll {before} -> {after} (revision-bump reset)"
    );
    assert!(
        text_of(&app, ed).contains("x"),
        "the keystroke must still land in the document"
    );
}

// S1, bug 2's exact shape: the first wheel after an edit must not be eaten
// by the pending content revision (the reset ran in the same flush as the
// wheel and wiped the fresh offset).
#[test]
fn first_wheel_after_an_edit_is_not_eaten() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);

    // Edit, then wheel in the SAME drain — no settling wait in between.
    app.send_key("x");
    app.wheel(0.0, 60.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);

    let after = scroll_of(&app, ed);
    assert!(
        (after - 60.0).abs() < 0.001,
        "the first wheel after an edit must scroll: scroll_y = {after}"
    );

    // And further wheels keep accumulating.
    app.wheel(0.0, 60.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    let more = scroll_of(&app, ed);
    assert!(
        (more - 120.0).abs() < 0.001,
        "wheels accumulate after an edit: scroll_y = {more}"
    );
}

// S1: undo restores a prior value through the span-write rail
// (`set_spans_preserve_cursor`) — a span write, not an external
// replacement. The scroll must stay put (the restored caret is on the
// visible line, so caret-follow is a no-op too).
#[test]
fn undo_does_not_reset_the_scroll() {
    let (mut app, ed) = setup(EDITOR_RUT);

    // Focus, scroll deep, caret onto the visible line, type there.
    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);
    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);
    app.send_key("x");
    app.wait_for_timeout(ZERO);

    let before = scroll_of(&app, ed);
    assert!(
        before > 100.0,
        "fixture precondition: scrolled, got {before}"
    );

    app.send_key_with_modifiers("z", false, true);
    app.wait_for_timeout(ZERO);

    let after = scroll_of(&app, ed);
    assert!(
        (after - before).abs() < 0.001,
        "undo snapped the scroll {before} -> {after} (span write must not reset)"
    );
    assert_eq!(
        text_of(&app, ed).matches('x').count(),
        0,
        "undo must still restore the prior text"
    );
}

// The pinned DESIRED behavior: a programmatic replacement (the playground's
// case switch, via `tctrl_set_text`) DOES reset the scroll to the top.
#[test]
fn replacing_the_text_resets_the_scroll() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    assert!(scroll_of(&app, ed) > 100.0, "fixture precondition");

    app.call_rut_entry("load_case", 0, 0.0).unwrap();
    app.wait_for_timeout(ZERO);

    assert_eq!(scroll_of(&app, ed), 0.0, "a new case opens at the top");
}

// S2: after the case switch settles, the editor must wheel immediately —
// no dead wheel while some cached max is stale.
#[test]
fn wheel_after_a_case_switch_scrolls_immediately() {
    let (mut app, ed) = setup(EDITOR_RUT);

    app.call_rut_entry("load_case", 0, 0.0).unwrap();
    app.wait_for_timeout(ZERO);

    app.wheel(0.0, 60.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);

    let after = scroll_of(&app, ed);
    assert!(
        (after - 60.0).abs() < 0.001,
        "wheel after a case switch must scroll: scroll_y = {after}"
    );
}

// S2, the recreation journey: loading a fresh module boots a brand-new
// editor element (fresh scroll cells) — the immediate wheel must still
// scroll (max_scroll_y valid before any wheel dispatch post-recreation).
#[test]
fn reloaded_module_editor_wheels_immediately() {
    let (mut app, _ed) = setup(EDITOR_RUT);

    // A full module reload: the old tree is torn down, a fresh editor boots.
    app.load_rut_module(EDITOR_RUT).unwrap();
    app.wait_for_timeout(ZERO);
    let ed = find_editable(&app);

    app.wheel(0.0, 60.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);

    let after = scroll_of(&app, ed);
    assert!(
        (after - 60.0).abs() < 0.001,
        "wheel immediately after recreation must scroll: scroll_y = {after}"
    );
}

// S3 + bug 1's journey: scroll down, drag a selection — the covered byte
// range must be the dragged one, and a follow-up click places the caret on
// the clicked line. Byte math is exact via caret-rect calibration (the
// fixture's lines are 7 bytes each: line i starts at 7*i).
#[test]
fn drag_and_click_after_scroll_hit_the_right_bytes() {
    let (mut app, ed) = setup(EDITOR_RUT);

    // Focus at the top-left; caret lands on line 0 at byte 0.
    app.click(101.0, 5.0);
    app.wait_for_timeout(ZERO);
    assert_eq!(
        cursor_of(&app, ed),
        0,
        "top-left click places the caret at byte 0"
    );

    // Calibrate the monospace cell from the caret rect.
    let (x0, y0, _, _) = app.focused_cursor_rect().unwrap();
    app.send_key("ArrowRight");
    app.wait_for_timeout(ZERO);
    let (x1, _, _, _) = app.focused_cursor_rect().unwrap();
    let cw = x1 - x0;
    app.send_key("ArrowDown");
    app.wait_for_timeout(ZERO);
    let (_, y1, _, _) = app.focused_cursor_rect().unwrap();
    let line_h = y1 - y0;
    assert!(
        cw > 3.0 && line_h > 5.0,
        "calibration: cw={cw} line_h={line_h}"
    );

    // Scroll down exactly three lines: content line 3 sits at the top of
    // the viewport (its content y == the scroll offset → screen y0).
    app.wheel(0.0, 3.0 * line_h, 100.0, 30.0);
    app.wait_for_timeout(ZERO);

    // Drag from line 3 col 2 to line 4 col 2 → bytes 23..30. Clicks target
    // cell CENTERS (col + 0.5): a click exactly on a cell edge is ambiguous
    // under nearest-stop rounding.
    app.pointer_down(x0 + 2.5 * cw, y0 + 0.5 * line_h);
    app.wait_for_timeout(ZERO); // a flush between down and move (the real gap)
    app.pointer_move(x0 + 2.5 * cw, y0 + 1.5 * line_h);
    app.wait_for_timeout(ZERO);
    app.pointer_up(x0 + 2.5 * cw, y0 + 1.5 * line_h);
    app.wait_for_timeout(ZERO);

    let (anchor, end) = selection_of(&app, ed);
    assert_eq!(
        (anchor, end),
        (23, 30),
        "the drag after scrolling must select the dragged range"
    );
    assert_eq!(cursor_of(&app, ed), 30, "the caret rides the drag end");

    // Bug 1's punchline: a plain click after the drag must still move the
    // caret. Col 5 (well outside the 5px multi-click radius of the drag's
    // pointer-down — closer would classify as a double-click word-select):
    // line 3 col 5 → byte 26.
    app.click(x0 + 5.5 * cw, y0 + 0.5 * line_h);
    app.wait_for_timeout(ZERO);
    assert_eq!(
        cursor_of(&app, ed),
        26,
        "click after scroll+drag places the caret on the clicked line"
    );
}
