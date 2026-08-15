use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn stack_children_overlap() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("stack-basic").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let stack = tree.get_element(root.children[0].as_element_id()).unwrap();
        assert_eq!(stack.kind().unwrap(), ElementKind::new("tur_stack"));
        assert_eq!(stack.children.len(), 2);

        let sb1 = tree.get_element(stack.children[0].as_element_id()).unwrap();
        let sb2 = tree.get_element(stack.children[1].as_element_id()).unwrap();
        assert_eq!(sb1.kind().unwrap(), ElementKind::new("tur_container"));
        assert_eq!(sb2.kind().unwrap(), ElementKind::new("tur_container"));

        (sb1.id, sb2.id)
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();

    let sb1_node = rt.get_element(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let sb2_node = rt.get_element(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.offset.x, 0.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 0.0);
}
