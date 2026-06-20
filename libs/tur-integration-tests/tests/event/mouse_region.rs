use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::TextElement;
use tur_integration_tests::TurTestApp;

fn build(app: &mut TurTestApp) -> ElementNodeId {
    app.load_bundle("mouse-region-cursor").unwrap();
    app.query_element(&["mr-state"]).unwrap()
}

fn find_region(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let col = tree.get(root.children[0]).unwrap();
    col.children[0]
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
        let _ = app.tick();
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

    app.render();

    // No cursor before pointer enters.
    assert_eq!(app.take_current_cursor(), None);

    let (cx, cy) = app
        .get_element_absolute_bounds(region_id)
        .unwrap()
        .center();
    app.pointer_move(cx, cy);
    flush(&mut app);

    // Cursor applied.
    assert_eq!(
        app.take_current_cursor(),
        Some("col-resize".to_string()),
        "MouseRegion cursor should apply on hover"
    );
    assert_eq!(span_content(&app, text_id), "entered");

    // Move away — cursor resets to default.
    app.pointer_move(999.0, 999.0);
    flush(&mut app);

    assert_eq!(
        app.take_current_cursor(),
        Some("default".to_string()),
        "cursor should reset to default when leaving the region"
    );
    assert_eq!(span_content(&app, text_id), "exited");
}
