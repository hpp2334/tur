use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn positioned_with_left_top() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-basic").unwrap();

    let pos_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let stack = tree.get_element(root.children[0].as_element_id()).unwrap();
        assert_eq!(stack.kind().unwrap(), ElementKind::new("tur_stack"));
        assert_eq!(stack.children.len(), 1);

        let positioned = tree.get_element(stack.children[0].as_element_id()).unwrap();
        assert_eq!(
            positioned.kind().unwrap(),
            ElementKind::new("tur_positioned")
        );
        assert_eq!(positioned.children.len(), 1);

        let sb = tree
            .get_element(positioned.children[0].as_element_id())
            .unwrap();
        assert_eq!(sb.kind().unwrap(), ElementKind::new("tur_container"));

        positioned.id
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 10.0);
    assert_eq!(pos_node.computed_layout.offset.y, 20.0);
}
