use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn positioned_with_left_top() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-basic").unwrap();

    let pos_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let stack = tree.get(root.children[0]).unwrap();
        assert_eq!(
            stack.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_stack")
        );
        assert_eq!(stack.children.len(), 1);

        let positioned = tree.get(stack.children[0]).unwrap();
        assert_eq!(
            positioned.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_positioned")
        );
        assert_eq!(positioned.children.len(), 1);

        let sb = tree.get(positioned.children[0]).unwrap();
        assert_eq!(
            sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        positioned.id
    };

    app.render();
    let rt = app.element_tree();
    let pos_node = rt.get(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 10.0);
    assert_eq!(pos_node.computed_layout.offset.y, 20.0);
}
