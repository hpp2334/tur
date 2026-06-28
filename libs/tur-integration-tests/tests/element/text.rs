use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn text_content_and_measurement() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-basic").unwrap();

    let text_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_paragraph")
        );
        container.id
    };

    app.render();
    let rt = app.element_tree();
    let text_node = rt.get_element(ElementNodeId::new(text_id.as_u64())).unwrap();
    let layout = &text_node.computed_layout;
    assert!(layout.size.width > 0.0, "text width should be positive");
    assert!(layout.size.height > 0.0, "text height should be positive");
}

#[test]
fn text_empty_content_zero_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-empty-content").unwrap();

    app.render();
    let rt = app.element_tree();
    let root = rt.root_element().unwrap();
    let text_node = rt.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
    let layout = &text_node.computed_layout;
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn text_font_size_affects_height() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-font-size").unwrap();

    app.render();
    let rt = app.element_tree();
    let root = rt.root_element().unwrap();
    let small = rt.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
    let large = rt.get_element(ElementNodeId::new(root.children[1].as_u64())).unwrap();
    assert!(
        large.computed_layout.size.height > small.computed_layout.size.height,
        "28px ({}) should be taller than 14px ({})",
        large.computed_layout.size.height,
        small.computed_layout.size.height,
    );
}

#[test]
fn text_wrapping_with_narrow_constraints() {
    let mut app = TurTestApp::new(80.0, 600.0).unwrap();
    app.load_bundle("text-wrapping").unwrap();

    app.render();
    let rt = app.element_tree();
    let root = rt.root_element().unwrap();
    let text_node = rt.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
    let layout = &text_node.computed_layout;
    assert!(
        layout.size.height > 30.0,
        "wrapped text should span multiple lines: height={}",
        layout.size.height,
    );
    assert!(
        layout.size.width <= 80.0,
        "width should not exceed 80px constraint: width={}",
        layout.size.width,
    );
}

#[test]
fn text_wrapping_vs_no_wrapping() {
    let mut app_narrow = TurTestApp::new(60.0, 600.0).unwrap();
    app_narrow.load_bundle("text-wrapping").unwrap();
    app_narrow.render();
    let wrapped_height = {
        let rt = app_narrow.element_tree();
        let root = rt.root_element().unwrap();
        rt.get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap()
            .computed_layout
            .size
            .height
    };

    let mut app_wide = TurTestApp::new(800.0, 600.0).unwrap();
    app_wide.load_bundle("text-wrapping").unwrap();
    app_wide.render();
    let unwrapped_height = {
        let rt = app_wide.element_tree();
        let root = rt.root_element().unwrap();
        rt.get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap()
            .computed_layout
            .size
            .height
    };

    assert!(
        wrapped_height > unwrapped_height,
        "wrapped ({}) should be taller than unwrapped ({})",
        wrapped_height,
        unwrapped_height,
    );
}

#[test]
fn text_in_column_vertical_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-in-column").unwrap();

    app.render();
    let rt = app.element_tree();
    let root = rt.root_element().unwrap();
    let col = rt.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
    let t1 = rt.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
    let t2 = rt.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();

    assert_eq!(t1.computed_layout.offset.y, 0.0);
    assert!(
        t2.computed_layout.offset.y >= t1.computed_layout.size.height,
        "second text (y={}) should start below first text (height={})",
        t2.computed_layout.offset.y,
        t1.computed_layout.size.height,
    );
}
