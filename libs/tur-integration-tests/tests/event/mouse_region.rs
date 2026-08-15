use tur_engine::builtin_plugins::text::elements::TextElement;
use tur_engine::core::element::ElementNodeId;
use tur_engine::core::platform::Cursor;
use tur_integration_tests::TurTestApp;

fn build(app: &mut TurTestApp) -> ElementNodeId {
    app.load_bundle("mouse-region-cursor").unwrap();
    let id = app.query_element(&["mr-state"]).unwrap();
    id.as_element_id()
}

fn find_region(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root_element().unwrap();
    let col = tree.get_element(root.children[0].as_element_id()).unwrap();
    col.children[0].as_element_id()
}

fn span_content(app: &TurTestApp, id: ElementNodeId) -> String {
    app.with_element(id, |e| {
        e.cast::<TextElement>()
            .map(|tc| {
                tc.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn flush(app: &mut TurTestApp) {
    for _ in 0..6 {
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }
}

#[test]
fn mouse_region_callbacks_registered() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build(&mut app);
    let region_id = find_region(&app);

    assert!(
        app.has_mouse_region_callbacks(region_id),
        "MouseRegion should report region callbacks"
    );
    assert!(
        !app.has_mouse_region_callbacks(text_id),
        "Text node should not report region callbacks"
    );
}

#[test]
fn hover_cursor_applies() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build(&mut app);
    let region_id = find_region(&app);

    app.wait_for_timeout(std::time::Duration::ZERO);

    // No cursor before pointer enters.
    assert_eq!(app.take_current_cursor(), None);

    let (cx, cy) = app.get_element_absolute_bounds(region_id).unwrap().center();
    app.pointer_move(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    flush(&mut app);

    // Cursor applied.
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::ColResize),
        "MouseRegion cursor should apply on hover"
    );
    assert_eq!(span_content(&app, text_id), "entered");

    // Move away — cursor resets to default.
    app.pointer_move(999.0, 999.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    flush(&mut app);

    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::Default),
        "cursor should reset to default when leaving the region"
    );
    assert_eq!(span_content(&app, text_id), "exited");
}

#[test]
fn reactive_cursor_updates() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("mouse-region-reactive-cursor").unwrap();
    let region_id = find_region(&app);

    app.wait_for_timeout(std::time::Duration::ZERO);

    // No cursor before pointer enters.
    assert_eq!(app.take_current_cursor(), None);

    let (cx, cy) = app.get_element_absolute_bounds(region_id).unwrap().center();

    // Initial cursor resolves to the source's initial value ("pointer").
    app.pointer_move(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    flush(&mut app);
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::Pointer),
        "reactive cursor should resolve to the source's initial value"
    );

    // Move away — resets to default.
    app.pointer_move(999.0, 999.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    flush(&mut app);
    assert_eq!(app.take_current_cursor(), Some(Cursor::Default));

    // Flip the cursor source and re-hover — the cursor must update after a
    // relayout re-resolves the prop.
    app.eval_js("globalThis.__setCursor('ew-resize')");
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.pointer_move(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    flush(&mut app);
    assert_eq!(
        app.take_current_cursor(),
        Some(Cursor::EwResize),
        "reactive cursor should update after the source changes"
    );
}
