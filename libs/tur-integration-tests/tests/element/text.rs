use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;
use tur_shared::ElementKind;

#[test]
fn text_content_and_measurement() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-basic").unwrap();

    let text_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let text = tree.get(root.children[0]).unwrap();
        assert_eq!(text.kind, ElementKind::Text);
        assert_eq!(text.prop_str("content"), Some("Hello"));
        assert_eq!(text.prop_f64("fontSize"), Some(14.0));
        text.id.as_u64()
    };

    app.with_render_tree(|rt| {
        let text_node = rt.get(RenderNodeId::new(text_id)).unwrap();
        assert_eq!(text_node.computed_layout.size.width, 42.0);
        assert_eq!(text_node.computed_layout.size.height, 16.8);
    });
}
