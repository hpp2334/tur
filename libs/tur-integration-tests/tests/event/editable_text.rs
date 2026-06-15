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
