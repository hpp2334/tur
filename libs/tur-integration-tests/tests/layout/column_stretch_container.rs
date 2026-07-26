use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// Repro for the reported layout bug:
/// Column(crossAlignment: Stretch) > Container(padding, color, no explicit
/// size) > Text. The Container should stretch to the Column's full width and
/// size its height from the text + padding. It must NOT collapse to zero.
#[test]
fn column_stretch_container_with_text_is_visible() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-stretch-container-text").unwrap();

    let container_id = ElementNodeId::new(app.query_element(&["container"]).unwrap().as_u64());
    let text_id = ElementNodeId::new(app.query_element(&["text"]).unwrap().as_u64());

    app.render();
    let rt = app.element_tree();

    let container = rt.get_element(container_id).unwrap();
    eprintln!(
        "[stretch] container size = {:?}, offset = {:?}",
        container.computed_layout.size, container.computed_layout.offset
    );
    assert!(
        container.computed_layout.size.width > 0.0,
        "container width collapsed to {} (stretch should fill cross axis)",
        container.computed_layout.size.width,
    );
    assert!(
        container.computed_layout.size.height > 0.0,
        "container height collapsed to {} (text + padding should give height)",
        container.computed_layout.size.height,
    );

    let text = rt.get_element(text_id).unwrap();
    eprintln!(
        "[stretch] text size = {:?}, offset = {:?}",
        text.computed_layout.size, text.computed_layout.offset
    );
    assert!(
        text.computed_layout.size.height > 0.0,
        "text height collapsed to {}",
        text.computed_layout.size.height,
    );
}

/// Repro #5: Container sized from a Text child, wrapped in Expanded inside a
/// Column. The Expanded should fill the main axis; the container must be
/// visible (non-zero size).
#[test]
fn column_expanded_container_with_text_is_visible() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-expanded-container-text").unwrap();

    let container_id = ElementNodeId::new(app.query_element(&["container"]).unwrap().as_u64());
    let text_id = ElementNodeId::new(app.query_element(&["text"]).unwrap().as_u64());

    app.render();
    let rt = app.element_tree();

    let container = rt.get_element(container_id).unwrap();
    eprintln!(
        "[expanded] container size = {:?}, offset = {:?}",
        container.computed_layout.size, container.computed_layout.offset
    );
    assert!(
        container.computed_layout.size.height > 0.0,
        "container height collapsed to {}",
        container.computed_layout.size.height,
    );
    assert!(
        container.computed_layout.size.width > 0.0,
        "container width collapsed to {}",
        container.computed_layout.size.width,
    );

    let text = rt.get_element(text_id).unwrap();
    assert!(
        text.computed_layout.size.height > 0.0,
        "text height collapsed to {}",
        text.computed_layout.size.height,
    );
}

/// Repro #6: Container sized from a Text child, wrapped in
/// ScrollView(Vertical) > Column. The container must be laid out at its
/// intrinsic size and be visible.
#[test]
fn scroll_view_container_with_text_is_visible() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("scroll-container-text").unwrap();

    let container_id = ElementNodeId::new(app.query_element(&["container"]).unwrap().as_u64());
    let text_id = ElementNodeId::new(app.query_element(&["text"]).unwrap().as_u64());

    app.render();
    let rt = app.element_tree();

    let container = rt.get_element(container_id).unwrap();
    eprintln!(
        "[scroll] container size = {:?}, offset = {:?}",
        container.computed_layout.size, container.computed_layout.offset
    );
    assert!(
        container.computed_layout.size.height > 0.0,
        "container height collapsed to {}",
        container.computed_layout.size.height,
    );

    let text = rt.get_element(text_id).unwrap();
    assert!(
        text.computed_layout.size.height > 0.0,
        "text height collapsed to {}",
        text.computed_layout.size.height,
    );
}

