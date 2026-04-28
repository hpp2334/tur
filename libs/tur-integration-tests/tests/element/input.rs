use tur_engine::core::element::ElementKind;
use tur_engine::elements::InputElement;
use tur_integration_tests::TurTestApp;

fn build_input_basic() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-basic").unwrap();
    app
}

fn build_input_typing() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-typing").unwrap();
    app
}

fn find_input_id(app: &TurTestApp) -> tur_engine::core::element::ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let container = tree.get(root.children[0]).unwrap();
    assert_eq!(
        container.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_container")
    );
    let input_node = tree.get(container.children[0]).unwrap();
    assert_eq!(
        input_node.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_input")
    );
    input_node.id
}

fn focus_input(app: &mut TurTestApp, input_id: tur_engine::core::element::ElementNodeId) {
    let tree = app.element_tree();
    let input_node = tree.get(input_id).unwrap();
    let layout = input_node.computed_layout;
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current = Some(input_id);
    while let Some(cid) = current {
        if let Some(n) = tree.get(cid) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else {
            break;
        }
    }
    drop(tree);

    let click_x = abs_x + layout.size.width / 2.0;
    let click_y = abs_y + layout.size.height / 2.0;
    app.click(click_x, click_y);
}

#[test]
fn input_create_and_kind() {
    let app = build_input_basic();
    let input_id = find_input_id(&app);
    assert!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().is_empty())
                .unwrap_or(false)
        })
        .unwrap_or(false),
        "input should be empty initially"
    );
}

#[test]
fn input_layout_positive_size() {
    let mut app = build_input_basic();
    let input_id = find_input_id(&app);

    app.render();
    let rt = app.element_tree();
    let input_node = rt.get(input_id).unwrap();
    let layout = &input_node.computed_layout;
    assert!(
        layout.size.height > 0.0,
        "input height should be positive: got {}",
        layout.size.height,
    );
}

#[test]
fn input_set_text_via_bridge() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-set-text").unwrap();

    let input_id = find_input_id(&app);

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "hello"
    );
}

#[test]
fn input_placeholder_layout_when_empty() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-placeholder").unwrap();

    let input_id = find_input_id(&app);

    app.render();
    let rt = app.element_tree();
    let input_node = rt.get(input_id).unwrap();
    let layout = &input_node.computed_layout;
    assert!(
        layout.size.height > 0.0,
        "placeholder should drive positive height: got {}",
        layout.size.height,
    );
}

#[test]
fn input_click_to_focus() {
    let mut app = build_input_basic();
    let input_id = find_input_id(&app);
    app.render();

    assert!(
        !app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.is_focused())
                .unwrap_or(false)
        })
        .unwrap_or(false),
        "input should not be focused before click"
    );

    focus_input(&mut app, input_id);

    assert!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.is_focused())
                .unwrap_or(false)
        })
        .unwrap_or(false),
        "input should be focused after click"
    );
}

#[test]
fn input_key_type_character() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("H");
    app.send_key("i");

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "Hi"
    );
}

#[test]
fn input_backspace_deletes() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("Backspace");

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "ab"
    );
}

#[test]
fn input_cursor_movement() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");

    let pos_after_ab = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.cursor_position())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(pos_after_ab, 2, "cursor should be at end after typing 'ab'");

    app.send_key("ArrowLeft");
    let pos_after_left = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.cursor_position())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(pos_after_left, 1, "cursor should move left to 1");

    app.send_key("Home");
    let pos_after_home = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.cursor_position())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(pos_after_home, 0, "cursor should be at 0 after Home");

    app.send_key("End");
    let pos_after_end = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.cursor_position())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(pos_after_end, 2, "cursor should be at end after End");
}

#[test]
fn input_on_input_callback() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("input-callback").unwrap();

    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("x");
    app.send_key("y");

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "xy"
    );
}

#[test]
fn shift_arrow_left_creates_selection() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");

    app.send_key_with_modifiers("ArrowLeft", true, false);

    let (anchor, end) = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| (i.selection_anchor(), i.selection_end()))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert!(anchor != end, "selection should exist after Shift+ArrowLeft");
    assert_eq!(
        anchor, 3,
        "anchor should be at original cursor position 3"
    );
    assert_eq!(end, 2, "end should have moved left to 2");
}

#[test]
fn shift_arrow_right_creates_selection() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");

    app.send_key("Home");
    app.send_key_with_modifiers("ArrowRight", true, false);

    let (anchor, end) = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| (i.selection_anchor(), i.selection_end()))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert!(anchor != end, "selection should exist after Shift+ArrowRight");
    assert_eq!(anchor, 0, "anchor should be at 0");
    assert_eq!(end, 1, "end should have moved right to 1");
}

#[test]
fn ctrl_a_selects_all() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("h");
    app.send_key("e");
    app.send_key("l");
    app.send_key("l");
    app.send_key("o");

    app.send_key_with_modifiers("a", false, true);

    let (anchor, end) = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| (i.selection_anchor(), i.selection_end()))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(anchor, 0, "selection should start at 0");
    assert_eq!(end, 5, "selection should end at 5 (len of 'hello')");
    assert!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.has_selection())
                .unwrap_or(false)
        })
        .unwrap_or(false),
        "has_selection should be true"
    );
}

#[test]
fn backspace_deletes_selection() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("d");

    app.send_key_with_modifiers("ArrowLeft", true, false);
    app.send_key_with_modifiers("ArrowLeft", true, false);

    let text_before = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(text_before, "abcd", "text should be unchanged before delete");

    app.send_key("Backspace");

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "ab"
    );
}

#[test]
fn typing_replaces_selection() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("d");

    app.send_key_with_modifiers("ArrowLeft", true, false);
    app.send_key_with_modifiers("ArrowLeft", true, false);

    app.send_key("x");

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "abx"
    );
}

#[test]
fn arrow_cancels_selection() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");

    app.send_key_with_modifiers("ArrowLeft", true, false);
    assert!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.has_selection())
                .unwrap_or(false)
        })
        .unwrap_or(false),
        "should have selection after Shift+ArrowLeft"
    );

    app.send_key("ArrowLeft");

    assert!(
        !app
            .with_element(input_id, |e| {
                e.cast::<InputElement>()
                    .map(|i| i.has_selection())
                    .unwrap_or(false)
            })
            .unwrap_or(false),
        "selection should be cleared after plain ArrowLeft"
    );
}

#[test]
fn shift_home_selects_to_start() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");

    app.send_key_with_modifiers("Home", true, false);

    let (anchor, end) = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| (i.selection_anchor(), i.selection_end()))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(anchor, 3, "anchor should be at 3");
    assert_eq!(end, 0, "end should be at 0");
}

#[test]
fn shift_end_selects_to_end() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");

    app.send_key("Home");
    app.send_key_with_modifiers("End", true, false);

    let (anchor, end) = app
        .with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| (i.selection_anchor(), i.selection_end()))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert_eq!(anchor, 0, "anchor should be at 0");
    assert_eq!(end, 3, "end should be at 3");
}

#[test]
fn delete_key_deletes_selection() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("d");

    app.send_key_with_modifiers("ArrowLeft", true, false);
    app.send_key_with_modifiers("ArrowLeft", true, false);

    app.send_key("Delete");

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "ab"
    );
}

#[test]
fn selected_text_returns_range() {
    let mut app = build_input_typing();
    let input_id = find_input_id(&app);
    app.render();
    focus_input(&mut app, input_id);

    app.send_key("a");
    app.send_key("b");
    app.send_key("c");
    app.send_key("d");

    app.send_key_with_modifiers("ArrowLeft", true, false);
    app.send_key_with_modifiers("ArrowLeft", true, false);

    assert_eq!(
        app.with_element(input_id, |e| {
            e.cast::<InputElement>()
                .map(|i| i.selected_text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "cd"
    );
}
