use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn text_content_and_measurement() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-basic").unwrap();

    let text_id = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let text = tree.get(root.children[0]).unwrap();
        assert_eq!(text.element.kind().as_str(), "tur_text");
        text.id.as_u64()
    };

    let rt = app.render_tree();
    let rt = rt.borrow();
    let text_node = rt.get(RenderNodeId::new(text_id)).unwrap();
    assert_eq!(text_node.computed_layout.size.width, 42.0);
    assert_eq!(text_node.computed_layout.size.height, 16.8);
}
