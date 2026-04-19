use tur_element::ElementKind;
use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn sized_box_fixed_dimensions() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("sized-box-basic").unwrap();

    let sb_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let sb = tree.get(root.children[0]).unwrap();
        assert_eq!(sb.kind, ElementKind::Container);
        assert_eq!(sb.children.len(), 1);

        let text = tree.get(sb.children[0]).unwrap();
        assert_eq!(text.kind, ElementKind::Text);

        sb.id.as_u64()
    };

    let rt = app.render_tree();
    let sb_node = rt.get(RenderNodeId::new(sb_id)).unwrap();
    assert_eq!(sb_node.computed_layout.size.width, 100.0);
    assert_eq!(sb_node.computed_layout.size.height, 50.0);
}
