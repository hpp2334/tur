use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::elements::ScrollViewElement;
use tur_text::elements::EditableTextElement;
use tur_integration_tests::TurTestApp;

/// Locate the `EditableTextElement` nested under the element tagged with the
/// given `queryKey` (Input puts the queryKey on its Container wrapper; the
/// editable text is that container's first child).
fn find_editable_under(app: &TurTestApp, key: &[&str]) -> ElementNodeId {
    let container_id = app.query_element(key).expect("queryKey not found");
    let container_id = ElementNodeId::new(container_id.as_u64());
    let tree = app.element_tree();
    let container = tree.get_element(container_id).unwrap();
    for cid in container.children.iter().copied() {
        let node = tree.get_element(ElementNodeId::new(cid.as_u64())).unwrap();
        if node
            .element
            .as_ref()
            .map(|e| e.kind() == ElementKind::new("tur_editable_text"))
            .unwrap_or(false)
        {
            return ElementNodeId::new(cid.as_u64());
        }
    }
    panic!("no tur_editable_text under queryKey {:?}", key);
}

/// Walk ancestors from `id` to find the enclosing `ScrollViewElement`, if any.
fn find_ancestor_scroll_view(app: &TurTestApp, id: ElementNodeId) -> Option<ElementNodeId> {
    let tree = app.element_tree();
    let mut current = tree.get_element(id).unwrap().parent;
    while let Some(cid) = current {
        let node = tree.get_element(ElementNodeId::new(cid.as_u64())).unwrap();
        if node
            .element
            .as_ref()
            .map(|e| e.cast::<ScrollViewElement>().is_some())
            .unwrap_or(false)
        {
            return Some(ElementNodeId::new(cid.as_u64()));
        }
        current = node.parent;
    }
    None
}

fn find_editable_text_id(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root_element().unwrap();
    let child = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
    let inner = tree.get_element(ElementNodeId::new(child.children[0].as_u64())).unwrap();
    let kind = inner.element.as_ref().unwrap().kind();
    if kind == ElementKind::new("tur_editable_text") {
        inner.id
    } else {
        tree.get_element(ElementNodeId::new(inner.children[0].as_u64())).unwrap().id
    }
}

fn focus_editable(app: &mut TurTestApp, id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
}

fn get_cursor_pos(app: &TurTestApp, id: ElementNodeId) -> usize {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.cursor_position())
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

fn get_text(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn get_selection(app: &TurTestApp, id: ElementNodeId) -> (usize, usize) {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.selection())
            .unwrap_or((0, 0))
    })
    .unwrap_or((0, 0))
}

#[test]
fn cursor_stays_in_middle_after_typing() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);
    app.render();

    app.send_key("a");
    app.send_key("b");
    app.render();
    assert_eq!(get_text(&app, input_id), "ab");
    assert_eq!(get_cursor_pos(&app, input_id), 2);

    app.send_key("ArrowLeft");
    app.render();
    assert_eq!(get_cursor_pos(&app, input_id), 1);

    app.send_key("X");
    app.render();
    assert_eq!(get_text(&app, input_id), "aXb");
    assert_eq!(get_cursor_pos(&app, input_id), 2);
}

#[test]
fn cursor_preserved_after_rerender() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-cursor-mid").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);
    app.render();

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.render();
    assert_eq!(get_text(&app, input_id), "abc");
    assert_eq!(get_cursor_pos(&app, input_id), 3);

    app.send_key("ArrowLeft");
    app.render();
    assert_eq!(get_cursor_pos(&app, input_id), 2);

    app.eval_js("globalThis.__setCursorMidTick(1)");
    app.render();

    assert_eq!(get_text(&app, input_id), "abc", "text unchanged after rerender");
    assert_eq!(
        get_cursor_pos(&app, input_id), 2,
        "cursor should stay at 2 after rerender, not jump to end"
    );
}

#[test]
fn delete_selected_first_character() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);
    app.render();

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.render();
    assert_eq!(get_text(&app, input_id), "abc");

    app.send_key("Home");
    app.render();
    assert_eq!(get_cursor_pos(&app, input_id), 0, "cursor at start after Home");

    app.send_key_with_modifiers("ArrowRight", true, false);
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    assert_eq!(anchor, 0, "anchor at 0");
    assert_eq!(end, 1, "end at 1 (selected 'a')");

    app.send_key("Backspace");
    app.render();

    assert_eq!(get_text(&app, input_id), "bc", "first char should be deleted");
    assert_eq!(get_cursor_pos(&app, input_id), 0, "cursor should be at 0");
}

#[test]
fn drag_select_release_outside_then_backspace() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (left, top, bottom) = (bounds.left, bounds.top, bounds.bottom);
    let cy = top + (bottom - top) / 2.0;

    focus_editable(&mut app, input_id);
    app.render();

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("d");
    app.render();
    assert_eq!(get_text(&app, input_id), "abcd");

    app.pointer_down(left + 1.0, cy);
    app.pointer_move(left + 20.0, cy);
    app.pointer_up(left + 300.0, cy + 200.0);
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    eprintln!("after drag+release outside: anchor={}, end={}", anchor, end);
    assert_ne!(anchor, end, "should still have selection after releasing outside");

    assert!(app.focused_element() == Some(input_id), "editable text should still be focused after pointer_up outside");

    app.send_key("Backspace");
    app.render();

    let text = get_text(&app, input_id);
    eprintln!("after backspace: text='{}'", text);
    assert_ne!(text, "abcd", "text should change after backspace with selection");
}

#[test]
fn click_before_first_char_then_type_and_select_delete() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (left, top, bottom) = (bounds.left, bounds.top, bounds.bottom);
    let cy = top + (bottom - top) / 2.0;

    app.pointer_down(left, cy);
    app.pointer_up(left, cy);
    app.render();

    app.send_key("a");
    app.send_key("b");
    app.render();
    assert_eq!(get_text(&app, input_id), "ab");

    app.send_key("Home");
    app.render();
    assert_eq!(get_cursor_pos(&app, input_id), 0);

    app.send_key_with_modifiers("ArrowRight", true, false);
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    eprintln!("selection: anchor={}, end={}", anchor, end);
    assert_eq!(anchor, 0);
    assert_eq!(end, 1);

    app.send_key("Backspace");
    app.render();

    let text = get_text(&app, input_id);
    eprintln!("after backspace: text='{}'", text);
    assert_eq!(text, "b", "should delete 'a', got '{}'", text);
}

#[test]
fn mouse_drag_select_then_backspace() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (left, top, bottom) = (bounds.left, bounds.top, bounds.bottom);
    let cy = top + (bottom - top) / 2.0;

    focus_editable(&mut app, input_id);
    app.render();

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("d");
    app.send_key("e");
    app.send_key("f");
    app.render();
    assert_eq!(get_text(&app, input_id), "abcdef");

    app.pointer_down(left + 1.0, cy);
    app.pointer_move(left + 30.0, cy);
    app.pointer_up(left + 30.0, cy);
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    eprintln!("after drag: anchor={}, end={}", anchor, end);
    assert_ne!(anchor, end, "should have selection after drag");
    let selected_len = end.max(anchor) - end.min(anchor);
    let expected_remaining = "abcdef".len() - selected_len;

    app.send_key("Backspace");
    app.render();

    let text_after = get_text(&app, input_id);
    eprintln!("text after backspace: '{}'", text_after);
    assert_eq!(text_after.len(), expected_remaining, "selected {} chars but got '{}' after delete", selected_len, text_after);
}

#[test]
fn multiline_drag_select_across_lines() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-multiline").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (left, top) = (bounds.left, bounds.top);

    focus_editable(&mut app, input_id);
    app.render();

    // Type three lines: "aaaa\nbbbb\ncccc"
    for _ in 0..4 { app.send_key("a"); }
    app.send_key("Enter");
    for _ in 0..4 { app.send_key("b"); }
    app.send_key("Enter");
    for _ in 0..4 { app.send_key("c"); }
    app.render();
    assert_eq!(get_text(&app, input_id), "aaaa\nbbbb\ncccc");

    // Compute y for line 0 (top) and line 2 (two lines down).
    // With fontSize 14, line height ≈ 14 * 1.2 = 16.8. The text occupies the
    // top portion of the (tall) element; subsequent lines sit at predictable
    // y offsets from `top`.
    let line_h = 14.0 * 1.2;
    let y_line0 = top + line_h * 0.5;
    let y_line2 = top + line_h * 2.5;

    // Drag from line 0 to line 2.
    app.pointer_down(left + 5.0, y_line0);
    app.pointer_move(left + 5.0, y_line2);
    app.pointer_up(left + 5.0, y_line2);
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    eprintln!(
        "after multi-line drag: anchor={}, end={}, text='{}'",
        anchor,
        end,
        get_text(&app, input_id)
    );
    assert_ne!(anchor, end, "should have selection spanning multiple lines");

    // The selection should cover at least one newline character, meaning it
    // spans more than a single line.
    let (s, e) = if anchor < end { (anchor, end) } else { (end, anchor) };
    let selected = &get_text(&app, input_id)[s..e];
    assert!(
        selected.contains('\n'),
        "multi-line drag should select across newlines; got '{}'",
        selected
    );

    // Backspace should delete the selected range.
    let expected_remaining: String = {
        let full = get_text(&app, input_id);
        let mut out = String::new();
        out.push_str(&full[..s]);
        out.push_str(&full[e..]);
        out
    };
    app.send_key("Backspace");
    app.render();
    assert_eq!(
        get_text(&app, input_id),
        expected_remaining,
        "text should be the unselected remainder after backspace",
    );
}

#[test]
fn multiline_drag_select_batched_events() {
    // Reproduces the browser scenario: queue down + several moves + up without
    // flushing in between, then flush once. This is how the wasm frame loop
    // processes events that arrive between animation frames.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-multiline").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (left, top) = (bounds.left, bounds.top);

    focus_editable(&mut app, input_id);
    app.render();

    for _ in 0..4 { app.send_key("a"); }
    app.send_key("Enter");
    for _ in 0..4 { app.send_key("b"); }
    app.send_key("Enter");
    for _ in 0..4 { app.send_key("c"); }
    app.render();
    assert_eq!(get_text(&app, input_id), "aaaa\nbbbb\ncccc");

    let line_h = 14.0 * 1.2;
    let y_line0 = top + line_h * 0.5;
    let y_line2 = top + line_h * 2.5;

    // Queue all drag events WITHOUT flushing between them — this mirrors how
    // the browser's frame loop sees multiple mouse events that arrive between
    // animation frames.
    app.pointer_down_no_flush(left + 5.0, y_line0);
    for frac in [1, 2, 3] {
        let y = y_line0 + (y_line2 - y_line0) * (frac as f64 / 4.0);
        app.pointer_move_no_flush(left + 5.0, y);
    }
    app.pointer_move_no_flush(left + 5.0, y_line2);
    app.pointer_up_no_flush(left + 5.0, y_line2);
    // Single flush processes all events at once.
    app.pump().unwrap();
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    eprintln!(
        "after batched multi-line drag: anchor={}, end={}, text='{}'",
        anchor,
        end,
        get_text(&app, input_id)
    );
    assert_ne!(
        anchor, end,
        "should have multi-line selection even when events are processed in one batch",
    );
    let (s, e) = if anchor < end { (anchor, end) } else { (end, anchor) };
    let selected = &get_text(&app, input_id)[s..e];
    assert!(
        selected.contains('\n'),
        "batched multi-line drag should select across newlines; got '{}'",
        selected
    );
}

const ONKEY_BUNDLE: &str = r#"
import { mutate, render, Input } from "builtin:tur/std";
globalThis.__keyHit = "";
globalThis.__ctrlHeld = "false";
const onKey = mutate((_storeCtx, ev) => {
    globalThis.__keyHit = ev.key;
    globalThis.__ctrlHeld = String(ev.ctrl);
});
globalThis.__ctrl = new globalThis.TextEditingController({ onKeyDown: onKey });
render(Input({ controller: globalThis.__ctrl, fontSize: 20, width: 200, height: 44 }));
"#;

/// Regression: the controller's `onKeyDown` listener must fire on every
/// keydown. Previously the field was stored but never dispatched, which left
/// the playground's Cmd+S shortcut (and any controller onKeyDown handler)
/// completely dead.
#[test]
fn controller_on_key_down_fires_on_keydown() {
    let mut app = TurTestApp::new(300.0, 100.0).unwrap();
    app.eval_module_source(ONKEY_BUNDLE).unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);
    app.render();

    // Ctrl+S must not insert text but must still fire onKeyDown.
    assert_eq!(get_text(&app, input_id), "");
    app.send_key_with_modifiers("s", false, true);
    app.render();
    assert_eq!(
        app.eval_js("globalThis.__keyHit"),
        "s",
        "onKeyDown must fire for Ctrl+S",
    );
    assert_eq!(
        app.eval_js("globalThis.__ctrlHeld"),
        "true",
        "modifier flag must be forwarded to onKeyDown",
    );
    assert_eq!(get_text(&app, input_id), "", "Ctrl+S must not insert text");

    // A plain printable key must also fire onKeyDown (and insert the char).
    app.send_key("a");
    app.render();
    assert_eq!(
        app.eval_js("globalThis.__keyHit"),
        "a",
        "onKeyDown must also fire for normal typing",
    );
    assert_eq!(get_text(&app, input_id), "a");
}

const SPANS_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{ content: "hello" }]);
render(Input({
    controller: globalThis.__ctrl,
    fontSize: 20,
    width: 200,
    height: 44,
}));
"#;

/// `setSpansPreserveCursor` must keep the caret where it is across a
/// re-tokenize pass (e.g. live syntax highlighting); the legacy `setSpans`
/// must continue to reset the caret to end-of-text.
#[test]
fn set_spans_preserve_cursor_keeps_caret() {
    let mut app = TurTestApp::new(300.0, 100.0).unwrap();
    app.eval_module_source(SPANS_BUNDLE).unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);
    app.render();

    // Normalize the caret to a known position: Home → 0, then right twice → 2.
    app.send_key("Home");
    app.render();
    app.send_key("ArrowRight");
    app.send_key("ArrowRight");
    app.render();
    assert_eq!(get_text(&app, input_id), "hello");
    assert_eq!(get_cursor_pos(&app, input_id), 2);

    // Re-tokenize with colored spans while preserving the caret.
    app.eval_js(
        r#"globalThis.__ctrl.setSpansPreserveCursor([
            { content: "he", color: { r: 255, g: 80, b: 80, a: 255 } },
            { content: "llo", color: { r: 80, g: 200, b: 120, a: 255 } },
        ]);"#,
    );
    app.render();
    assert_eq!(get_text(&app, input_id), "hello");
    assert_eq!(
        get_cursor_pos(&app, input_id),
        2,
        "caret must stay at 2 after preserve-cursor re-tokenize",
    );

    // Contrast: the legacy `setSpans` resets the caret to end-of-text (5).
    app.eval_js(r#"globalThis.__ctrl.setSpans([{ content: "hello" }]);"#);
    app.render();
    assert_eq!(
        get_cursor_pos(&app, input_id),
        5,
        "setSpans resets caret to end of text",
    );
}

// ---------------------------------------------------------------------------
// Click → caret → Backspace regression tests.
//
// The reported playground bug: click to move the caret, then press Backspace,
// and a *different* character is removed (not the one immediately left of the
// caret). To assert precisely without hard-coding a font's pixel advance, each
// test first *calibrates* the monospace char width and line height from the
// focused element's caret rect, then clicks at an exact byte boundary.
// ---------------------------------------------------------------------------

/// Absolute `(x, y_top, height)` of the focused element's caret.
fn caret_rect(app: &TurTestApp) -> (f64, f64, f64) {
    let (x, y, _w, h) = app.focused_cursor_rect().expect("focused caret rect");
    (x, y, h)
}

/// Calibrate the monospace char width for the currently-focused editable text.
/// Requires the caret to be at byte 0 on entry; leaves the caret at byte 1.
fn calibrate_char_width(app: &mut TurTestApp) -> f64 {
    let (x0, _, _) = caret_rect(app);
    app.send_key("ArrowRight");
    app.render();
    let (x1, _, _) = caret_rect(app);
    let cw = x1 - x0;
    assert!(cw > 3.0, "char width implausibly small: {cw}");
    cw
}

const CLICK_SINGLE_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{ content: "hello" }]);
render(Input({
    controller: globalThis.__ctrl,
    fontFamily: "monospace",
    fontSize: 20,
    width: 400,
    height: 44,
}));
"#;

// Mirrors the playground code editor: syntax-highlighted spans with different
// colors, which forces parley to emit MULTIPLE glyph runs on a single line.
// This is the one configuration difference vs. the single-span tests above.
const CLICK_SPANS_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([
    { content: "import", color: { r: 200, g: 120, b: 50, a: 255 } },
    { content: " {", color: { r: 80, g: 80, b: 80, a: 255 } },
]);
render(Input({
    controller: globalThis.__ctrl,
    fontFamily: "monospace",
    fontSize: 20,
    width: 400,
    height: 44,
}));
"#;

const CLICK_MULTI_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{ content: "abc\ndef\nghi" }]);
render(Input({
    controller: globalThis.__ctrl,
    multiline: true,
    fontFamily: "monospace",
    fontSize: 20,
    width: 400,
    height: 200,
}));
"#;

/// Click in the middle of a single-line field should place the caret at the
/// clicked boundary, and Backspace must then delete the character immediately
/// to its left (not some other character).
#[test]
fn click_places_caret_then_backspace_deletes_left_char() {
    let mut app = TurTestApp::new(500.0, 200.0).unwrap();
    app.eval_module_source(CLICK_SINGLE_BUNDLE).unwrap();
    app.render();

    let id = find_editable_text_id(&app);
    focus_editable(&mut app, id);
    app.render();
    app.send_key("Home");
    app.render();
    assert_eq!(get_cursor_pos(&app, id), 0);

    let cw = calibrate_char_width(&mut app);
    let (_, y_top, h) = caret_rect(&app);
    // After calibration the caret sits at byte 1; recover the line-0 caret x.
    app.send_key("Home");
    app.render();
    let (x0, _, _) = caret_rect(&app);
    eprintln!("single-line: x0={x0} cw={cw}");

    // Click at the left edge of char 3 → caret at byte 3 ("he|llo").
    let target = 3usize;
    let cy = y_top + h / 2.0;
    app.click(x0 + cw * target as f64, cy);
    app.render();

    let caret = get_cursor_pos(&app, id);
    eprintln!("after click for byte {target}: caret={caret}");
    assert_eq!(caret, target, "click should place caret at byte {target}");

    // Backspace deletes byte 2 (the 'l' immediately left of the caret).
    app.send_key("Backspace");
    app.render();
    eprintln!(
        "after backspace: text='{}' caret={}",
        get_text(&app, id),
        get_cursor_pos(&app, id)
    );
    assert_eq!(
        get_text(&app, id),
        "helo",
        "backspace must delete the char immediately left of the caret"
    );
    assert_eq!(get_cursor_pos(&app, id), 2);
}

/// Multiline variant: click on the *second* line and Backspace must delete the
/// char left of the caret on that line. Exercises the `byte_index_at_xy`
/// (y-based line selection) path, which the single-line test does not.
#[test]
fn click_places_caret_on_second_line_then_backspace_deletes_left_char() {
    let mut app = TurTestApp::new(500.0, 300.0).unwrap();
    app.eval_module_source(CLICK_MULTI_BUNDLE).unwrap();
    app.render();

    let id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(id).unwrap();

    // Position the caret at line 0, col 0 by clicking the top-left of the
    // field. (focus_editable clicks the element center, which on a 3-line
    // field lands on line 2 — so click explicitly.)
    app.click(bounds.left + 1.0, bounds.top + 1.0);
    app.render();

    let (x0, y0, h) = caret_rect(&app);
    let cw = calibrate_char_width(&mut app);
    // Caret is now at byte 1 on line 0. Move onto line 1 to read its top y.
    app.send_key("ArrowDown");
    app.render();
    let (_, y1, _) = caret_rect(&app);
    let line_h = y1 - y0;
    eprintln!("multi: x0={x0} cw={cw} y0={y0} y1={y1} line_h={line_h}");
    assert!(line_h > 5.0, "line height implausibly small: {line_h}");

    // Line 1 is "def" and starts at byte 4 ("abc\n" = 4 bytes). Click col 1
    // ("d|ef") → caret at byte 5.
    let column = 1usize;
    let target = 4usize + column;
    let cy = y1 + h / 2.0;
    app.click(x0 + cw * column as f64, cy);
    app.render();

    let caret = get_cursor_pos(&app, id);
    eprintln!("after click line1 col{column}: caret={caret} (expected {target})");
    assert_eq!(
        caret, target,
        "click on line 1 col {column} should place caret at byte {target}"
    );

    // Backspace deletes byte 4 ('d'), the char immediately left of the caret.
    app.send_key("Backspace");
    app.render();
    eprintln!(
        "after backspace: text='{}'",
        get_text(&app, id).replace('\n', "\\n")
    );
    assert_eq!(
        get_text(&app, id),
        "abc\nef\nghi",
        "backspace must delete 'd' (the char left of the caret on line 1)"
    );
    assert_eq!(get_cursor_pos(&app, id), 4);
}

/// The playground editor renders syntax-highlighted code: many adjacent spans
/// with *different colors*, which makes parley emit multiple glyph runs on a
/// single line. This must not corrupt click hit-testing: clicking at a given
/// byte boundary must still place the caret there, and Backspace must still
/// delete the char immediately left of it.
#[test]
fn click_with_multi_color_spans_places_caret_correctly() {
    let mut app = TurTestApp::new(500.0, 200.0).unwrap();
    app.eval_module_source(CLICK_SPANS_BUNDLE).unwrap();
    app.render();

    let id = find_editable_text_id(&app);
    let bounds = app.get_element_absolute_bounds(id).unwrap();
    // Click top-left → byte 0 ("import" run start).
    app.click(bounds.left + 1.0, bounds.top + 1.0);
    app.render();

    let (_, y_top, h) = caret_rect(&app);
    let cw = calibrate_char_width(&mut app);
    app.send_key("Home");
    app.render();
    let (x0, _, _) = caret_rect(&app);
    eprintln!("multi-span: x0={x0} cw={cw}");

    // "import {" → click at the left edge of byte 4 ("impo|rt").
    let target = 4usize;
    let cy = y_top + h / 2.0;
    app.click(x0 + cw * target as f64, cy);
    app.render();

    let caret = get_cursor_pos(&app, id);
    eprintln!("after click for byte {target}: caret={caret}");
    assert_eq!(
        caret, target,
        "click should place caret at byte {target} even with multi-color spans"
    );

    // Backspace deletes byte 3 ('o') → "imprt {".
    app.send_key("Backspace");
    app.render();
    eprintln!(
        "after backspace: text='{}' caret={}",
        get_text(&app, id),
        get_cursor_pos(&app, id)
    );
    assert_eq!(
        get_text(&app, id),
        "imprt {",
        "backspace must delete the char immediately left of the caret"
    );
    assert_eq!(get_cursor_pos(&app, id), 3);
}

// Four adjacent spans with DIFFERENT colors → parley emits 4 glyph runs on one
// line. Reproduces the playground "Buy gro|ceries" bug: clicking inside a LATER
// run (not the first) must still place the caret at the clicked byte.
const CLICK_FOUR_SPAN_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([
    { content: "AAAA", color: { r: 200, g: 120, b: 50, a: 255 } },
    { content: "BBBB", color: { r: 80, g: 200, b: 120, a: 255 } },
    { content: "CCCC", color: { r: 120, g: 80, b: 200, a: 255 } },
    { content: "DDDD", color: { r: 200, g: 200, b: 80, a: 255 } },
]);
render(Input({
    controller: globalThis.__ctrl,
    fontFamily: "monospace",
    fontSize: 20,
    width: 400,
    height: 44,
}));
"#;

#[test]
fn click_in_later_run_places_caret_correctly() {
    let mut app = TurTestApp::new(500.0, 200.0).unwrap();
    app.eval_module_source(CLICK_FOUR_SPAN_BUNDLE).unwrap();
    app.render();

    let id = find_editable_text_id(&app);
    focus_editable(&mut app, id);
    app.render();
    app.send_key("Home");
    app.render();
    assert_eq!(get_cursor_pos(&app, id), 0);

    let cw = calibrate_char_width(&mut app);
    app.send_key("Home");
    app.render();
    let (x0, y_top, h) = caret_rect(&app);
    let cy = y_top + h / 2.0;

    // Click at each byte boundary across all 4 runs.
    for target in [0usize, 2, 4, 6, 8, 10, 12, 14, 16] {
        app.click(x0 + cw * target as f64, cy);
        app.render();
        let caret = get_cursor_pos(&app, id);
        eprintln!("four-span: target={target} caret={caret}");
        assert_eq!(
            caret, target,
            "click at byte {target} should place caret there (4-span line)"
        );
    }
}

// A zero-length span carrying a color (the playground's `buildHighlightSpans`
// can emit these from adjacent/zero-width lexer tokens) must NOT crash parley.
// Regression for the "click todolist state.ts → panic" bug: an empty style
// range (`start == end`) triggered
// `assertion failed: style_run.range.start < style_run.range.end`.
const EMPTY_SPAN_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([
    { content: "ab", color: { r: 200, g: 120, b: 50, a: 255 } },
    { content: "", color: { r: 80, g: 200, b: 120, a: 255 } },
    { content: "cd", color: { r: 120, g: 80, b: 200, a: 255 } },
]);
render(Input({
    controller: globalThis.__ctrl,
    fontFamily: "monospace",
    fontSize: 20,
    width: 400,
    height: 44,
}));
"#;

#[test]
fn empty_colored_span_does_not_panic() {
    let mut app = TurTestApp::new(500.0, 200.0).unwrap();
    app.eval_module_source(EMPTY_SPAN_BUNDLE).unwrap();
    // Rendering must not panic despite the empty-color span producing an
    // empty (start == end) style range.
    app.render();

    let id = find_editable_text_id(&app);
    // The empty span contributes no text; the visible content is "abcd".
    assert_eq!(get_text(&app, id), "abcd");
}

// Mirrors the playground editor: a multiline `Input` INSIDE a `ScrollView`,
// scrolled down so the clicked line is only reachable after scrolling. This is
// the exact configuration the reported bug was seen in.
//
// The content is intentionally long enough that scrolling `2 * line_height`
// stays well within the scroll-view's clamp range (content_height -
// viewport_height). With a 100px viewport and 16px lines, ~12 lines (=192px)
// leaves ~92px of scroll headroom — enough that the 2-line scroll in the test
// body never hits the clamp.
const CLICK_SCROLLED_BUNDLE: &str = r#"
import { render, ScrollView, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{
    content: "L0AAAA\nL1BBBB\nL2CCCC\nL3DDDD\nL4EEEE\nL5FFFF\nL6GGGG\nL7HHHH\nL8IIII\nL9JJJJ\nL10KKK\nL11LLL",
}]);
render(ScrollView({
    child: Input({
        controller: globalThis.__ctrl,
        multiline: true,
        fontFamily: "monospace",
        fontSize: 16,
        queryKey: ["scrolled-input"],
    }),
}));
"#;

/// Clicking a line that is only visible AFTER scrolling must place the caret on
/// that (content-space) line — the scroll offset must be folded into the click
/// coordinate. A regression here would land the caret on an early line instead.
#[test]
fn click_on_scrolled_line_places_caret_on_that_line() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(CLICK_SCROLLED_BUNDLE).unwrap();
    app.render();

    let id = find_editable_under(&app, &["scrolled-input"]);
    let sv_id = find_ancestor_scroll_view(&app, id).expect("editable inside a ScrollView");

    // At scroll 0, click the top-left to focus + place caret at byte 0.
    let bounds = app.get_element_absolute_bounds(id).unwrap();
    app.click(bounds.left + 1.0, bounds.top + 1.0);
    app.render();

    // Calibrate char width and line height from the caret rect.
    let (x0, y0, _) = caret_rect(&app);
    app.send_key("ArrowRight");
    app.render();
    let (x1, _, _) = caret_rect(&app);
    let cw = x1 - x0;
    app.send_key("ArrowDown");
    app.render();
    let (_, y1, _) = caret_rect(&app);
    let line_h = y1 - y0;
    eprintln!("scrolled: x0={x0} cw={cw} y0={y0} line_h={line_h}");
    assert!(cw > 3.0 && line_h > 5.0);

    // Scroll down exactly two line heights → line 2 ("L2CCCC") sits at the
    // viewport top (its content y == scroll offset, so it renders at y0).
    let scroll_amount = 2.0 * line_h;
    let (cx, cy) = app.get_element_absolute_bounds(sv_id).unwrap().center();
    app.wheel(0.0, scroll_amount, cx, cy);
    app.render();
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert!((sv.scroll_offset() - scroll_amount).abs() < 0.5, "scrolled to {}", sv.scroll_offset());
    }).unwrap();

    // Line 2 starts at byte 14 ("L0AAAA\nL1BBBB\n" = 14 bytes). Click col 2
    // ("L2|CCCC") on the line now at the viewport top (screen y = y0).
    let line2_start = 14usize;
    let column = 2usize;
    let target = line2_start + column;
    app.click(x0 + cw * column as f64, y0 + line_h / 2.0);
    app.render();

    let caret = get_cursor_pos(&app, id);
    eprintln!("after scrolled click: caret={caret} (expected {target})");
    assert_eq!(
        caret, target,
        "click on the scrolled-into-view line 2 must place caret at byte {target}, \
         not on an early line (scroll offset must be applied)"
    );

    // Backspace deletes byte 15 ('2') → "L2CCCC" becomes "LCCCC".
    app.send_key("Backspace");
    app.render();
    eprintln!(
        "after backspace: text='{}'",
        get_text(&app, id).replace('\n', "\\n")
    );
    assert_eq!(
        get_text(&app, id),
        "L0AAAA\nL1BBBB\nLCCCC\nL3DDDD\nL4EEEE\nL5FFFF\nL6GGGG\nL7HHHH\nL8IIII\nL9JJJJ\nL10KKK\nL11LLL",
        "backspace must delete the char left of the caret on the scrolled line"
    );
    assert_eq!(get_cursor_pos(&app, id), 15);
}

// A long line that soft-wraps across multiple VISUAL lines (no `\n`, just soft
// wraps at word boundaries). The playground editor wraps long statements (e.g.
// todolist's `{ title: "Walk the dog", completed: true },`) and clicking a wrap
// continuation was reported to land the caret on the wrong character. parley
// only wraps at break opportunities, so the text MUST contain spaces (a bare
// digit string has none and overflows instead of wrapping). Bare `Input`
// root so the app's tight width bounds the editable.
const CLICK_SOFTWRAP_BUNDLE: &str = r#"
import { render, Input } from "builtin:tur/std";
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{
    content: "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega",
}]);
render(Input({
    controller: globalThis.__ctrl,
    multiline: true,
    fontFamily: "monospace",
    fontSize: 16,
    queryKey: ["softwrap-input"],
}));
"#;

#[test]
fn click_on_soft_wrapped_line_lands_on_correct_visual_segment() {
    let mut app = TurTestApp::new(120.0, 300.0).unwrap();
    app.eval_module_source(CLICK_SOFTWRAP_BUNDLE).unwrap();
    app.render();

    let id = find_editable_under(&app, &["softwrap-input"]);
    let bounds = app.get_element_absolute_bounds(id).unwrap();
    let (left, top) = (bounds.left, bounds.top);

    // Focus + read layout state (number of visual lines + true content height).
    app.click(left + 2.0, top + 2.0);
    app.render();
    let (x0, y0, _) = caret_rect(&app);
    let dev = app.dev_tool_get_element(id.into()).expect("editable dev node");
    let extra = |name: &str| -> f64 {
        dev.layout_extra
            .iter()
            .find(|(k, _)| *k == name)
            .and_then(|(_, v)| {
                if let tur_engine::core::elements::TraceValue::Num(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0.0)
    };
    let num_lines = extra("numLines") as usize;
    let lw = extra("layoutWidth") as f32;
    let lh = extra("layoutHeight") as f32;
    eprintln!("softwrap: num_lines={num_lines} layout_w={lw:.1} layout_h={lh:.1} x0={x0} y0={y0}");
    assert!(num_lines > 1, "spaced line should wrap (num_lines={num_lines})");
    let line_h = lh / num_lines as f32;
    app.send_key("ArrowRight");
    app.render();
    let (x1, _, _) = caret_rect(&app);
    let cw = x1 - x0;

    // Sweep x across visual line 0 (fixed y at its center). The caret must
    // increase monotonically with x.
    let mut prev = 0usize;
    let mut v0_bytes = Vec::new();
    for col in [0usize, 2, 4, 6, 8, 10] {
        let cx = x0 + cw * col as f64;
        app.click(cx, y0 + line_h as f64 * 0.5);
        app.render();
        let c = get_cursor_pos(&app, id);
        v0_bytes.push((col, c));
        assert!(
            c as isize >= prev as isize,
            "visual-0 click x monotonic broken: col {col} → caret {c} (prev {prev})"
        );
        prev = c;
    }
    eprintln!("softwrap: visual-0 x-sweep (col, caret) = {v0_bytes:?}");

    // Sweep y down the visual lines (fixed x at column 2). The caret must
    // strictly increase — each visual line is further into the text.
    let mut prev = 0usize;
    for ln in 0..num_lines.min(5) {
        app.click(x0 + cw * 2.0, y0 + line_h as f64 * (ln as f64 + 0.5));
        app.render();
        let c = get_cursor_pos(&app, id);
        eprintln!("softwrap: visual line {ln} (y={:.1}) → caret {c}", y0 + line_h as f64 * (ln as f64 + 0.5));
        assert!(
            c > prev,
            "y-sweep broken: visual line {ln} caret {c} not > prev {prev} — \
             wrap continuations must be separately hit-testable"
        );
        prev = c;
    }

    // Backspace on a wrap-continuation visual line deletes the char left of it.
    // Re-click visual line 2 at col 2 and backspace.
    let target_line = 2usize.min(num_lines - 1);
    app.click(x0 + cw * 2.0, y0 + line_h as f64 * (target_line as f64 + 0.5));
    app.render();
    let caret = get_cursor_pos(&app, id);
    let original = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega";
    let mut expected: String = original[..caret - 1].to_string();
    expected.push_str(&original[caret..]);
    app.send_key("Backspace");
    app.render();
    eprintln!("softwrap: after backspace at caret {caret} text='{}'", get_text(&app, id));
    assert_eq!(
        get_text(&app, id),
        expected,
        "backspace on a wrap continuation must delete the char left of the caret"
    );
}

// ---------------------------------------------------------------------------
// Multi-click classification (engine-side): PointerDoubleDown selects the
// word under the cursor, PointerTripleDown selects the whole line.
// ---------------------------------------------------------------------------

#[test]
fn double_click_selects_word() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);

    // Type a single word so the click position is forgiving — anywhere
    // over the glyphs selects the whole word.
    for ch in "hello".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();
    assert_eq!(get_text(&app, input_id), "hello");

    // The focus click already bumped the synthetic time. Push past the
    // 500ms multi-click window so the double_click's first pointer_down
    // is a fresh Single (its second pointer-down becomes the Double).
    for _ in 0..15 {
        app.bump_synthetic_time_ms_for_test(50);
    }

    // Click on the middle of the input — definitely over "hello".
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let click_x = (bounds.left + bounds.right) * 0.5;
    let click_y = (bounds.top + bounds.bottom) * 0.5;
    app.double_click(click_x, click_y);
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    let (lo, hi) = if anchor <= end { (anchor, end) } else { (end, anchor) };
    let selected = &get_text(&app, input_id)[lo..hi];
    assert_eq!(selected, "hello", "double-click should select the word under the cursor");
}

#[test]
fn single_click_after_double_click_collapses_selection() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app.render();

    let input_id = find_editable_text_id(&app);
    focus_editable(&mut app, input_id);
    for ch in "hello".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();

    // Push past the focus click's window so the double_click is fresh.
    for _ in 0..15 {
        app.bump_synthetic_time_ms_for_test(50);
    }

    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let click_x = (bounds.left + bounds.right) * 0.5;
    let click_y = (bounds.top + bounds.bottom) * 0.5;

    app.double_click(click_x, click_y);
    app.render();
    // Selection should be non-empty after double-click.
    let (a, e) = get_selection(&app, input_id);
    assert_ne!(a, e, "double-click should produce a selection");

    // Wait long enough that the next click is outside the multi-click
    // window — bump synthetic time past the 500ms threshold.
    for _ in 0..15 {
        app.bump_synthetic_time_ms_for_test(50);
    }
    app.pointer_down(click_x, click_y);
    app.render();
    let (a, e) = get_selection(&app, input_id);
    assert_eq!(a, e, "single click after the window must collapse the selection");
}
