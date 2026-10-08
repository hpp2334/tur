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
use tur_kit::{ Input, TextCtrl, UndoCtrl, mount, text_ctrl, undo_ctrl };

struct EditorCx {
    ctrl: TextCtrl,
    undo: UndoCtrl,
}

fn long_doc() -> str {
    let mut text = "";
    let mut i = 0;
    while (i < 40) {
        text = f"{text}line {i}\n";
        i = i + 1;
    }
    return text;
}

fn start() -> EditorCx {
    let ctrl = text_ctrl();
    ctrl.set_text(long_doc());
    let undo = undo_ctrl();
    let input = Input()
        .controller(ctrl)
        .undo(undo)
        .multiline(true)
        .font_family("monospace")
        .width_height(200.0, 60.0)
        .query_key("ed")
        .build();
    mount(input);
    return EditorCx { ctrl: ctrl, undo: undo };
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

// The case-switch rail: the playground loads a new case source into the
// SAME editor via the controller write (a true external replacement — the
// view must reopen at the top).
entry fn load_case(cx: opaque) {
    editor_cx(cx).ctrl.set_text(long_doc());
}
"#;

/// Mount the editor fixture and locate the `EditableTextElement` (the
/// `Input` view is a Container wrapper whose only child is the editable).
/// Boots through the context-crossing contract (`entry_start`) and
/// returns the held context token with the app.
fn setup(source: &str) -> (TurTestApp, ElementNodeId, u64) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(source).unwrap();
    let cx = app.call_rut_entry_opaque("entry_start").unwrap();
    app.wait_for_timeout(ZERO);
    let ed = find_editable(&app);
    (app, ed, cx)
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
    let (mut app, ed, _cx) = setup(EDITOR_RUT);

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
    let (mut app, ed, _cx) = setup(EDITOR_RUT);

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
    let (mut app, ed, _cx) = setup(EDITOR_RUT);

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
    let (mut app, ed, cx) = setup(EDITOR_RUT);

    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    assert!(scroll_of(&app, ed) > 100.0, "fixture precondition");

    app.call_rut_entry_cx("load_case", cx).unwrap();
    app.wait_for_timeout(ZERO);

    assert_eq!(scroll_of(&app, ed), 0.0, "a new case opens at the top");
}

// S2: after the case switch settles, the editor must wheel immediately —
// no dead wheel while some cached max is stale.
#[test]
fn wheel_after_a_case_switch_scrolls_immediately() {
    let (mut app, ed, cx) = setup(EDITOR_RUT);

    app.call_rut_entry_cx("load_case", cx).unwrap();
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
    let (mut app, _ed, _cx) = setup(EDITOR_RUT);

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
    let (mut app, ed, _cx) = setup(EDITOR_RUT);

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

// ── S4: the self-scroller's two remaining holes ─────────────────────────
//
// The boa editor scrolled through a pane-filling ScrollView (clamped to
// content − viewport, push_clip'd); the rut-era editable scrolls ITSELF.
// Two holes in the self-scroller, both user-reported against the served
// build:
//
//   1. A scrolled field paints its content translated by −scroll_y with
//      NO clip — lines above the viewport escape the editor's box (and
//      the pane, over the app header). The boa ScrollView wrapped every
//      child paint in `push_clip(Offset::ZERO, layout.size)`; the
//      self-scroller owns that duty now.
//   2. (rail below) content that fits must never scroll — the wheel
//      clamps to max_scroll_y = content − viewport, which is 0 when the
//      document fits.

use std::sync::Arc;

use tur_engine::core::clock::{Clock, FixedClock};
use tur_engine::core::element::NodeId;
use tur_engine::core::frame_env::FrameEnv;
use tur_engine::core::image_resource::{ImageManager, ImageResourceId};
use tur_engine::core::layout::{Geometry, Offset, Size};
use tur_engine::core::render::brush::{Brush, Color};
use tur_engine::core::render::Canvas;
use tur_engine::core::text::text_layout::TextLayoutData;
use vello_common::kurbo::Affine;

/// Two short lines in the 200×60 editor — the content fits with room to
/// spare, so the scrollable excess is 0 and a wheel must be a no-op.
const SHORT_RUT: &str = r#"
use tur_kit::{ Input, TextCtrl, UndoCtrl, mount, text_ctrl, undo_ctrl };

struct EditorCx {
    ctrl: TextCtrl,
    undo: UndoCtrl,
}

fn start() -> EditorCx {
    let ctrl = text_ctrl();
    ctrl.set_text("line 0\nline 1\n");
    let undo = undo_ctrl();
    let input = Input()
        .controller(ctrl)
        .undo(undo)
        .multiline(true)
        .font_family("monospace")
        .width_height(200.0, 60.0)
        .query_key("ed")
        .build();
    mount(input);
    return EditorCx { ctrl: ctrl, undo: undo };
}

entry fn entry_start() -> opaque {
    let cx = start();
    return opaque(cx);
}
"#;

#[test]
fn short_content_never_scrolls() {
    let (mut app, ed, _cx) = setup(SHORT_RUT);
    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);

    let max = max_scroll_of(&app, ed);
    assert_eq!(
        max, 0.0,
        "content that fits the viewport has no scrollable excess"
    );

    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    let y = scroll_of(&app, ed);
    assert_eq!(y, 0.0, "wheel on a fitting editor must not scroll");
}

fn max_scroll_of(app: &TurTestApp, ed: ElementNodeId) -> f64 {
    app.with_element(ed, |e| {
        e.cast::<EditableTextElement>().unwrap().max_scroll_y()
    })
    .unwrap()
}

// ── the clip recorder ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum PaintOp {
    NodeStart(u64),
    NodeEnd,
    FillText,
    PushClip { x: f64, y: f64, w: f64, h: f64 },
    PopClip,
    Other,
}

#[derive(Default)]
#[derive(Debug)]
struct ClipRecorder {
    ops: Vec<PaintOp>,
}

impl Canvas for ClipRecorder {
    fn fill_geometry(&mut self, _o: Offset, _g: &Geometry, _b: &Brush) {
        self.ops.push(PaintOp::Other);
    }
    fn stroke_geometry(&mut self, _o: Offset, _g: &Geometry, _c: &Color, _w: f64) {
        self.ops.push(PaintOp::Other);
    }
    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, _o: Offset, _l: &Arc<TextLayoutData>) {
        self.ops.push(PaintOp::FillText);
    }
    fn draw_image(&mut self, _r: ImageResourceId, _n: Size, _t: Affine) {}
    fn draw_shadow(
        &mut self,
        _o: Offset,
        _s: Size,
        _c: &Color,
        _r: f64,
        _b: f64,
        _so: (f64, f64),
    ) {
    }
    fn push_clip(&mut self, o: Offset, s: Size) {
        self.ops.push(PaintOp::PushClip {
            x: o.x,
            y: o.y,
            w: s.width,
            h: s.height,
        });
    }
    fn push_clip_geometry(&mut self, _o: Offset, _g: &Geometry) {
        self.ops.push(PaintOp::Other);
    }
    fn pop_clip(&mut self) {
        self.ops.push(PaintOp::PopClip);
    }
    fn push_opacity(&mut self, _o: f32) {}
    fn pop_opacity(&mut self) {}
    fn push_transform(&mut self, _t: Affine) {}
    fn pop_transform(&mut self) {}
    fn notify_node_entry(&mut self, id: ElementNodeId, _t: Affine, _s: Size) {
        self.ops.push(PaintOp::NodeStart(NodeId::from(id).as_u64()));
    }
    fn notify_node_exit(&mut self) {
        self.ops.push(PaintOp::NodeEnd);
    }
}

/// Paint the live tree into the recorder; return the editable's op segment
/// (between its NodeStart and the matching NodeEnd).
fn editable_paint_ops(app: &TurTestApp, ed: ElementNodeId) -> Vec<PaintOp> {
    app.with_tree(move |tree, focus| {
        let mut rec = ClipRecorder::default();
        let env = FrameEnv::new(std::rc::Rc::new(FixedClock::from_millis(0.0)));
        tree.paint(
            &mut rec,
            focus.focused(),
            &ImageManager::new(),
            env.paint_env(),
        );
        let start = rec
            .ops
            .iter()
            .position(|o| *o == PaintOp::NodeStart(NodeId::from(ed).as_u64()))
            .unwrap_or_else(|| panic!("the editable never painted"));
        let end = start
            + rec.ops[start..]
                .iter()
                .position(|o| *o == PaintOp::NodeEnd)
                .expect("unterminated node segment");
        rec.ops[start + 1..end].to_vec()
    })
    .unwrap()
}

#[test]
fn scrolled_paint_is_clipped_to_the_editor_bounds() {
    let (mut app, ed, _cx) = setup(EDITOR_RUT);
    app.click(100.0, 30.0);
    app.wait_for_timeout(ZERO);
    app.wheel(0.0, 200.0, 100.0, 30.0);
    app.wait_for_timeout(ZERO);
    let scrolled = scroll_of(&app, ed);
    assert!(scrolled > 50.0, "fixture precondition: scrolled, got {scrolled}");

    let seg = editable_paint_ops(&app, ed);
    let fill = seg
        .iter()
        .position(|o| *o == PaintOp::FillText)
        .expect("the editable paints its text layout");

    let clip = seg[..fill]
        .iter()
        .rev()
        .find(|o| matches!(o, PaintOp::PushClip { .. }))
        .copied()
        .expect(
            "the scrolled text fill must sit inside a push_clip — \
             without it the lines above the viewport paint outside the editor",
        );
    let PaintOp::PushClip { x, y, w, h } = clip else {
        unreachable!()
    };
    assert_eq!(
        (x, y),
        (0.0, 0.0),
        "the clip is the editor's own local bounds"
    );
    assert!(
        (w - 200.0).abs() < 0.5 && (h - 60.0).abs() < 0.5,
        "the clip is the editor viewport (200x60), got {w}x{h}"
    );
    assert!(
        seg[fill + 1..].contains(&PaintOp::PopClip),
        "the clip must be popped (balanced layers)"
    );
}
