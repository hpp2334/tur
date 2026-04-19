use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn stack_children_overlap() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("stack-basic").unwrap();

    let (sb1_id, sb2_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let stack = tree.get(root.children[0]).unwrap();
        assert_eq!(stack.element.name(), "tur_stack");
        assert_eq!(stack.children.len(), 2);

        let sb1 = tree.get(stack.children[0]).unwrap();
        let sb2 = tree.get(stack.children[1]).unwrap();
        assert_eq!(sb1.element.name(), "tur_container");
        assert_eq!(sb2.element.name(), "tur_container");

        (sb1.id.as_u64(), sb2.id.as_u64())
    };

    let rt = app.render_tree();
    let rt = rt.borrow();

    let sb1_node = rt.get(RenderNodeId::new(sb1_id)).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let sb2_node = rt.get(RenderNodeId::new(sb2_id)).unwrap();
    assert_eq!(sb2_node.computed_layout.offset.x, 0.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 0.0);
}
