use tur_element::ElementKind;
use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn container_with_padding() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("container-basic").unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let container = tree.get(root.children[0]).unwrap();
        assert_eq!(container.kind, ElementKind::Container);
        assert_eq!(container.children.len(), 1);

        let sb = tree.get(container.children[0]).unwrap();
        assert_eq!(sb.kind, ElementKind::Container);

        container.id.as_u64()
    };

    let rt = app.render_tree();
    let container_node = rt.get(RenderNodeId::new(container_id)).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 132.0);
    assert_eq!(container_node.computed_layout.size.height, 132.0);
}
