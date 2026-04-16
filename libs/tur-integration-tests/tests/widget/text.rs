use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;
use tur_widget::WidgetKind;

#[test]
fn text_content_and_measurement() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-basic").unwrap();

    let text_id = {
        let tree = app.widget_tree();
        let root = tree.root().unwrap();
        let text = tree.get(root.children[0]).unwrap();
        assert_eq!(text.kind, WidgetKind::Text);
        assert_eq!(text.prop_str("content"), Some("Hello"));
        assert_eq!(text.prop_f64("fontSize"), Some(14.0));
        text.id.as_u64()
    };

    let rt = app.render_tree();
    let text_node = rt.get(RenderNodeId::new(text_id)).unwrap();
    assert_eq!(text_node.computed_layout.size.width, 42.0);
    assert_eq!(text_node.computed_layout.size.height, 16.8);
    assert_eq!(text_node.text_content.as_deref(), Some("Hello"));
    assert_eq!(text_node.font_size, Some(14.0));
}
