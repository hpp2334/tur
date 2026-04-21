use tur_engine::core::element::ElementNodeId;
use tur_engine::core::event::EventKind;
use tur_engine::elements::TextElement;
use tur_integration_tests::TurTestApp;

fn build_clickable_text(app: &mut TurTestApp) -> ElementNodeId {
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        globalThis.__tur.setAttribute(ctx, root, "direction", 0);
        globalThis.__tur.setAttribute(ctx, root, "crossAlignment", 0);
        var col = globalThis.__tur.createFlex(ctx);
        globalThis.__tur.setAttribute(ctx, col, "direction", 0);
        globalThis.__tur.setAttribute(ctx, col, "crossAlignment", 0);
        globalThis.__tur.appendChild(ctx, root, col);

        var pi = globalThis.__tur.createPointerInteract(ctx);
        var text = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, text, "content", "before");
        globalThis.__tur.setAttribute(ctx, text, "queryKey", ["click-text"]);
        globalThis.__tur.appendChild(ctx, pi, text);
        globalThis.__tur.appendChild(ctx, col, pi);

        globalThis.__clickText = text;
        globalThis.__tur.setAttribute(ctx, pi, "onClick", function() {
            globalThis.__tur.setAttribute(ctx, globalThis.__clickText, "content", "after");
        });
    "#,
    )
    .unwrap();

    app.query_element(&["click-text"]).unwrap()
}

fn find_pointer_interact(app: &TurTestApp) -> ElementNodeId {
    let tree = app.element_tree();
    let tree = tree.borrow();
    let root = tree.root().unwrap();
    let col = tree.get(root.children[0]).unwrap();
    col.children[0]
}

#[test]
fn click_handler_registered() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    assert!(
        app.has_event_handler(pi_id, EventKind::Click),
        "PointerInteract should have onClick handler"
    );
    assert!(
        !app.has_event_handler(text_id, EventKind::Click),
        "Text node should not have onClick handler"
    );
}

#[test]
fn click_updates_text_content() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);
    let pi_id = find_pointer_interact(&app);

    app.render_tree();

    let tree = app.element_tree();
    let tree = tree.borrow();
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

    assert!(
        app.with_element(text_id, |e| {
            e.cast::<TextElement>()
                .map(|t| t.content() == "before")
                .unwrap_or(false)
        })
        .unwrap_or(false),
        "text should be 'before' before click"
    );

    let click_x = abs_x + pi_layout.size.width / 2.0;
    let click_y = abs_y + pi_layout.size.height / 2.0;
    app.click(click_x, click_y);

    assert_eq!(
        app.with_element(text_id, |e| {
            e.cast::<TextElement>()
                .map(|t| t.content().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "after",
        "text should be 'after' after click"
    );
}

#[test]
fn click_miss_does_not_update_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let text_id = build_clickable_text(&mut app);

    app.render_tree();

    assert_eq!(
        app.with_element(text_id, |e| {
            e.cast::<TextElement>()
                .map(|t| t.content().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "before"
    );

    app.click(999.0, 999.0);

    assert_eq!(
        app.with_element(text_id, |e| {
            e.cast::<TextElement>()
                .map(|t| t.content().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        "before",
        "text should remain 'before' after miss click"
    );
}
