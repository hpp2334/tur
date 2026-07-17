use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::stdlib::elements::EditableTextElement;
use tur_integration_tests::TurTestApp;

const INPUT_BUNDLE: &str = r#"
    import { createTextEditingController, render, Container, Input } from "builtin:tur/std";
    const controller = createTextEditingController({});
    render(Container({
        children: [
            Input({
                controller: controller,
                fontSize: 14,
                width: 200,
                height: 30,
                queryKey: ["input"],
            }),
        ],
    }));
"#;

fn find_editable(app: &TurTestApp) -> ElementNodeId {
    let container_id = app.query_element(&["input"]).expect("queryKey not found");
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
    panic!("no tur_editable_text under queryKey input");
}

fn get_selection(app: &TurTestApp, id: ElementNodeId) -> (usize, usize) {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.selection())
            .unwrap_or((0, 0))
    })
    .unwrap_or((0, 0))
}

fn get_cursor(app: &TurTestApp, id: ElementNodeId) -> usize {
    app.with_element(id, |e| {
        e.cast::<EditableTextElement>()
            .map(|el| el.cursor_position())
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

/// Right-click INSIDE the existing selection should preserve the selection so
/// the context-menu's Cut/Copy operate on it (matches native text fields).
#[test]
fn right_click_inside_selection_preserves_selection() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(INPUT_BUNDLE).unwrap();
    app.render();

    let input_id = find_editable(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (cx, cy) = bounds.center();

    // Focus and type "hello world" — wide enough to have a meaningful
    // selection range.
    app.click(cx, cy);
    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();

    // Select "world" (bytes 6..11) via Home + 6 ArrowRight + 5 Shift+ArrowRight.
    app.send_key("Home");
    for _ in 0..6 {
        app.send_key("ArrowRight");
    }
    for _ in 0..5 {
        app.send_key_with_modifiers("ArrowRight", true, false);
    }
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    assert_eq!((anchor, end), (6, 11),
        "precondition: 'world' should be selected");

    // Find a click position that lands inside the selected text region.
    // The selection spans roughly the right half of the input (bytes 6..11
    // out of 11), so click at ~75% of the input width.
    let inside_x = bounds.left + (bounds.right - bounds.left) * 0.75;

    // Right-click inside the selection. The DOM event order is
    // mousedown(button=2) → contextmenu → mouseup(button=2).
    app.right_click(inside_x, cy);

    let (anchor2, end2) = get_selection(&app, input_id);
    assert_eq!((anchor2, end2), (6, 11),
        "right-click inside selection should preserve selection (got ({anchor2}, {end2}))");
}

/// Right-click OUTSIDE the existing selection should move the caret to the
/// click position and collapse the selection (matches native text fields).
#[test]
fn right_click_outside_selection_moves_caret() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(INPUT_BUNDLE).unwrap();
    app.render();

    let input_id = find_editable(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (cx, cy) = bounds.center();

    app.click(cx, cy);
    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();

    // Select "world" (bytes 6..11).
    app.send_key("Home");
    for _ in 0..6 {
        app.send_key("ArrowRight");
    }
    for _ in 0..5 {
        app.send_key_with_modifiers("ArrowRight", true, false);
    }
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    assert_eq!((anchor, end), (6, 11));

    // Right-click near the start of the input — outside the selection.
    let outside_x = bounds.left + 5.0;
    app.right_click(outside_x, cy);

    let (anchor2, end2) = get_selection(&app, input_id);
    assert_eq!(anchor2, end2,
        "right-click outside selection should collapse the selection (got ({anchor2}, {end2}))");

    // Cursor should have moved to roughly byte 0 (the click was at the
    // leftmost edge). We don't assert exact byte position because character
    // hit-testing depends on layout, but the selection must be collapsed.
    let cursor = get_cursor(&app, input_id);
    assert!(cursor <= 3,
        "cursor should be near the start of the input, got {cursor}");
}

/// Left-click inside a selection should ALSO collapse it (existing behavior,
/// unchanged by the native-OS fix). This guards against regressions.
#[test]
fn left_click_inside_selection_collapses() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(INPUT_BUNDLE).unwrap();
    app.render();

    let input_id = find_editable(&app);
    let bounds = app.get_element_absolute_bounds(input_id).unwrap();
    let (cx, cy) = bounds.center();

    app.click(cx, cy);
    for ch in "hello world".chars() {
        app.send_key(&ch.to_string());
    }
    app.render();

    // Select "world".
    app.send_key("Home");
    for _ in 0..6 {
        app.send_key("ArrowRight");
    }
    for _ in 0..5 {
        app.send_key_with_modifiers("ArrowRight", true, false);
    }
    app.render();

    let (anchor, end) = get_selection(&app, input_id);
    assert_eq!((anchor, end), (6, 11));

    // Left-click at ~75% (inside selection).
    let inside_x = bounds.left + (bounds.right - bounds.left) * 0.75;
    app.click(inside_x, cy);

    let (anchor2, end2) = get_selection(&app, input_id);
    assert_eq!(anchor2, end2,
        "left-click inside selection should collapse the selection");
}
