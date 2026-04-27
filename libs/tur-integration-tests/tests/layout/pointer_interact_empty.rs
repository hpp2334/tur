use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn pointer_interact_no_child_zero_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-interact-empty").unwrap();

    let pi_id = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let pi = tree.get(root.children[0]).unwrap();
        assert_eq!(
            pi.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_pointer_interact")
        );
        assert_eq!(pi.children.len(), 0);
        pi.id
    };

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let pi_node = rt.get(pi_id).unwrap();
    assert_eq!(pi_node.computed_layout.size.width, 0.0);
    assert_eq!(pi_node.computed_layout.size.height, 0.0);
}
