use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn text_content_and_measurement() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-basic").unwrap();

    let text_id = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let text = tree.get(root.children[0]).unwrap();
        assert_eq!(
            text.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_text")
        );
        text.id
    };

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let text_node = rt.get(text_id).unwrap();
    let layout = &text_node.computed_layout;
    assert!(layout.size.width > 0.0, "text width should be positive");
    assert!(layout.size.height > 0.0, "text height should be positive");
}
