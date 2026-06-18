use tur_engine::core::element::ElementKind;
use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::EditableText;
use tur_integration_tests::TurTestApp;

fn find_editable_text_id(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let child = tree.get(root.children[0]).unwrap();
    let inner = tree.get(child.children[0]).unwrap();
    let kind = inner.element.as_ref().unwrap().kind();
    if kind == ElementKind::new("tur_editable_text") {
        inner.id
    } else {
        tree.get(inner.children[0]).unwrap().id
    }
}

fn focus_editable(app: &mut TurTestApp, id: ElementNodeId) {
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
}

fn get_cursor_pos(app: &TurTestApp, id: ElementNodeId) -> usize {
    app.with_element(id, |e| {
        e.cast::<EditableText>()
            .map(|el| el.cursor_position())
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

fn get_text(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<EditableText>()
            .map(|el| el.text())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn get_selection(app: &TurTestApp, id: ElementNodeId) -> (usize, usize) {
    app.with_element(id, |e| {
        e.cast::<EditableText>()
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

const ONKEY_BUNDLE: &str = r#"
const T = globalThis.__tur;
const ctx = T.__ctx;
globalThis.__keyHit = "";
globalThis.__ctrlHeld = "false";
const onKey = T.mutate(ctx, (_storeCtx, ev) => {
    globalThis.__keyHit = ev.key;
    globalThis.__ctrlHeld = String(ev.ctrl);
});
globalThis.__ctrl = new globalThis.TextEditingController({ onKeyDown: onKey });
T.render(ctx, T.InputEdgy(ctx, { controller: globalThis.__ctrl, fontSize: 20, width: 200, height: 44 }));
"#;

/// Regression: the controller's `onKeyDown` listener must fire on every
/// keydown. Previously the field was stored but never dispatched, which left
/// the playground's Cmd+S shortcut (and any controller onKeyDown handler)
/// completely dead.
#[test]
fn controller_on_key_down_fires_on_keydown() {
    let mut app = TurTestApp::new(300.0, 100.0).unwrap();
    app.load_bundle_source(ONKEY_BUNDLE).unwrap();
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
const T = globalThis.__tur;
const ctx = T.__ctx;
globalThis.__ctrl = new globalThis.TextEditingController();
globalThis.__ctrl.setSpans([{ content: "hello" }]);
T.render(ctx, T.InputEdgy(ctx, {
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
    app.load_bundle_source(SPANS_BUNDLE).unwrap();
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
