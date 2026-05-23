use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::TextContainerElement;
use tur_integration_tests::TurTestApp;

fn build_clickable_text(app: &mut TurTestApp) -> ElementNodeId {
    app.load_bundle("clickable-text").unwrap();
    app.query_element(&["click-text"]).unwrap()
}

fn find_pointer_interact(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let col = tree.get(root.children[0]).unwrap();
    col.children[0]
}

fn get_span_content(app: &TurTestApp, container_id: ElementNodeId) -> String {
    app.with_element(container_id, |e| {
        e.cast::<TextContainerElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[test]
fn click_handler_registered() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    assert!(
        app.has_click_handler(pi_id),
        "PointerInteract should have onClick handler"
    );
    assert!(
        !app.has_click_handler(text_id),
        "Text node should not have onClick handler"
    );
}

#[test]
fn click_updates_text_content() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render();

    assert_eq!(
        get_span_content(&app, text_id),
        "before",
        "text should be 'before' before click"
    );

    let (click_x, click_y) = app.get_element_absolute_bounds(pi_id).unwrap().center();
    app.click(click_x, click_y);

    assert_eq!(
        get_span_content(&app, text_id),
        "after",
        "text should be 'after' after click"
    );
}

#[test]
fn click_miss_does_not_update_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);

    app.render();

    assert_eq!(
        get_span_content(&app, text_id),
        "before"
    );

    app.click(999.0, 999.0);

    assert_eq!(
        get_span_content(&app, text_id),
        "before",
        "text should remain 'before' after miss click"
    );
}
