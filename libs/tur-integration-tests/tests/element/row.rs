use tur_element::ElementKind;
use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn row_basic_horizontal_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-basic").unwrap();

    let (row_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        assert_eq!(root.kind, ElementKind::Flex);
        assert_eq!(root.children.len(), 1);

        let row = tree.get(root.children[0]).unwrap();
        assert_eq!(row.kind, ElementKind::Flex);
        assert_eq!(row.children.len(), 2);

        let sb1 = tree.get(row.children[0]).unwrap();
        let sb2 = tree.get(row.children[1]).unwrap();
        assert_eq!(sb1.kind, ElementKind::Container);
        assert_eq!(sb2.kind, ElementKind::Container);

        (row.id.as_u64(), sb1.id.as_u64(), sb2.id.as_u64())
    };

    let rt = app.render_tree();

    let sb1_node = rt.get(RenderNodeId::new(sb1_id)).unwrap();
    assert_eq!(sb1_node.computed_layout.size.width, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);

    let sb2_node = rt.get(RenderNodeId::new(sb2_id)).unwrap();
    assert_eq!(sb2_node.computed_layout.size.width, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.x, 50.0);

    let row_node = rt.get(RenderNodeId::new(row_id)).unwrap();
    assert_eq!(row_node.computed_layout.size.width, 80.0);
}
