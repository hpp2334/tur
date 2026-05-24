use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::ParagraphElement;
use tur_integration_tests::TurTestApp;

fn build_pointer_region_text(app: &mut TurTestApp) -> ElementNodeId {
    app.load_bundle("pointer-region-text").unwrap();
    app.query_element(&["region-text"]).unwrap()
}

fn find_pointer_interact(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let col = tree.get(root.children[0]).unwrap();
    col.children[0]
}

fn get_span_content(app: &TurTestApp, container_id: ElementNodeId) -> String {
    app.with_element(container_id, |e| {
        e.cast::<ParagraphElement>()
            .map(|tc| tc.spans().iter().map(|s| s.text.as_str()).collect::<String>())
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
fn pointer_region_callbacks_registered() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_pointer_region_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    assert!(
        app.has_pointer_region_callbacks(pi_id),
        "PointerInteract should have pointer region callbacks"
    );
    assert!(
        !app.has_pointer_region_callbacks(text_id),
        "Text node should not have pointer region callbacks"
    );
}

#[test]
fn pointer_enter_updates_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_pointer_region_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render();

    assert_eq!(
        get_span_content(&app, text_id),
        "idle",
        "text should be 'idle' before pointer enters"
    );

    let (cx, cy) = app.get_element_absolute_bounds(pi_id).unwrap().center();
    app.pointer_move(cx, cy);
    flush(&mut app);

    assert_eq!(
        get_span_content(&app, text_id),
        "entered",
        "text should be 'entered' after pointer enters"
    );
}

#[test]
fn pointer_exit_updates_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_pointer_region_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render();

    let (cx, cy) = app.get_element_absolute_bounds(pi_id).unwrap().center();
    app.pointer_move(cx, cy);
    flush(&mut app);

    assert_eq!(
        get_span_content(&app, text_id),
        "entered",
        "text should be 'entered' while pointer is inside"
    );

    app.pointer_move(999.0, 999.0);
    flush(&mut app);

    assert_eq!(
        get_span_content(&app, text_id),
        "exited",
        "text should be 'exited' after pointer leaves"
    );
}

#[test]
fn pointer_move_within_does_not_exit() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_pointer_region_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render();

    let bounds = app.get_element_absolute_bounds(pi_id).unwrap();
    let cx = (bounds.left + bounds.right) / 2.0;
    let cy = (bounds.top + bounds.bottom) / 2.0;

    app.pointer_move(cx, cy);
    flush(&mut app);

    assert_eq!(
        get_span_content(&app, text_id),
        "entered",
        "text should be 'entered' after first move in"
    );

    app.pointer_move(cx + 5.0, cy + 5.0);
    flush(&mut app);

    assert_eq!(
        get_span_content(&app, text_id),
        "entered",
        "text should remain 'entered' while pointer stays inside"
    );

    app.pointer_move(cx - 3.0, cy - 3.0);
    flush(&mut app);

    assert_eq!(
        get_span_content(&app, text_id),
        "entered",
        "text should remain 'entered' on further moves inside"
    );
}

#[test]
fn no_events_without_callbacks() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-interact-basic").unwrap();
    app.render();

    app.pointer_move(50.0, 25.0);
    flush(&mut app);

    app.pointer_move(999.0, 999.0);
    flush(&mut app);
}
