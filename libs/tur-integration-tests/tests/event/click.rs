use tur_engine::core::element::ElementNodeId;
use tur_engine::core::gesture::ComposedGestureEventKind;
use tur_engine::elements::TextSpanElement;
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
    let tree = app.element_tree();
    let container = tree.get(container_id).unwrap();
    let span_id = container.children.first().copied();
    drop(tree);
    span_id
        .and_then(|id| {
            app.with_element(id, |e| {
                e.cast::<TextSpanElement>()
                    .map(|s| s.content().to_string())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
}

#[test]
fn click_handler_registered() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    assert!(
        app.has_event_handler(pi_id, ComposedGestureEventKind::Click),
        "PointerInteract should have onClick handler"
    );
    assert!(
        !app.has_event_handler(text_id, ComposedGestureEventKind::Click),
        "Text node should not have onClick handler"
    );
}

#[test]
fn click_updates_text_content() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render();

    let tree = app.element_tree();
    let pi_node = tree.get(pi_id).unwrap();
    let pi_layout = pi_node.computed_layout;

    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current = Some(pi_id);
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

    assert_eq!(
        get_span_content(&app, text_id),
        "before",
        "text should be 'before' before click"
    );

    let click_x = abs_x + pi_layout.size.width / 2.0;
    let click_y = abs_y + pi_layout.size.height / 2.0;
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
