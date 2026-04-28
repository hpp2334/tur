use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

fn get_container_and_spans(app: &TurTestApp) -> (tur_engine::core::element::ElementNodeId, Vec<tur_engine::core::element::ElementNodeId>) {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let container = tree.get(root.children[0]).unwrap();
    assert_eq!(
        container.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_text_container")
    );
    let spans: Vec<_> = container
        .children
        .iter()
        .filter_map(|&id| {
            tree.get(id)
                .and_then(|n| n.element.as_ref())
                .filter(|e| e.type_name() == "tur_text_span")
                .map(|_| id)
        })
        .collect();
    (container.id, spans)
}

#[test]
fn rich_text_single_span_equivalent_to_plain_text() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-single-span").unwrap();

    let (container_id, spans) = get_container_and_spans(&app);
    assert_eq!(spans.len(), 1);

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    let layout = &container_node.computed_layout;
    assert!(layout.size.width > 0.0, "text width should be positive");
    assert!(layout.size.height > 0.0, "text height should be positive");
}

#[test]
fn rich_text_multi_span_concatenates() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-multi-span").unwrap();

    let (container_id, spans) = get_container_and_spans(&app);
    assert_eq!(spans.len(), 3);

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    let layout = &container_node.computed_layout;
    assert!(
        layout.size.width > 50.0,
        "multi-span text should be wide: width={}",
        layout.size.width,
    );
    assert!(layout.size.height > 0.0);
}

#[test]
fn rich_text_bold_wider_than_normal() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-bold").unwrap();
    app.render();

    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let container = tree.get(root.children[0]).unwrap();
    assert_eq!(
        container.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_text_container")
    );

    let span_normal = tree.get(container.children[0]).unwrap();
    let span_bold = tree.get(container.children[1]).unwrap();

    assert_eq!(
        span_normal.element.as_ref().unwrap().type_name(),
        "tur_text_span"
    );
    assert_eq!(
        span_bold.element.as_ref().unwrap().type_name(),
        "tur_text_span"
    );

    assert!(container.computed_layout.size.width > 0.0);
}

#[test]
fn rich_text_italic_layout_positive() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-italic").unwrap();

    let (container_id, _) = get_container_and_spans(&app);
    app.render();

    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    assert!(
        container_node.computed_layout.size.width > 0.0,
        "italic text should have positive width"
    );
}

#[test]
fn rich_text_color_layout_positive() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-color").unwrap();

    let (container_id, spans) = get_container_and_spans(&app);
    assert_eq!(spans.len(), 3);

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    assert!(
        container_node.computed_layout.size.width > 0.0,
        "colored text should have positive width"
    );
}

#[test]
fn rich_text_font_size_mixed_height() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-font-size").unwrap();

    let (container_id, spans) = get_container_and_spans(&app);
    assert_eq!(spans.len(), 2);

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    let height = container_node.computed_layout.size.height;
    assert!(
        height >= 28.0,
        "container with 28px span should be at least 28px tall: height={}",
        height,
    );
}

#[test]
fn rich_text_empty_spans_zero_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-empty").unwrap();

    let (container_id, _) = get_container_and_spans(&app);
    app.render();

    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    assert_eq!(
        container_node.computed_layout.size.width, 0.0,
        "empty spans should produce zero width"
    );
    assert_eq!(
        container_node.computed_layout.size.height, 0.0,
        "empty spans should produce zero height"
    );
}

#[test]
fn rich_text_inheritance_uses_defaults() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("rich-text-inheritance").unwrap();

    let (container_id, spans) = get_container_and_spans(&app);
    assert_eq!(spans.len(), 2);

    app.render();
    let rt = app.element_tree();
    let container_node = rt.get(container_id).unwrap();
    assert!(
        container_node.computed_layout.size.width > 0.0,
        "inherited text should have positive width"
    );
    let height = container_node.computed_layout.size.height;
    assert!(
        height >= 20.0,
        "container with 20px default should be at least 20px tall: height={}",
        height,
    );
}
