use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn pointer_interact_passes_constraints_and_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-interact-basic").unwrap();

    let (pi_id, container_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        assert_eq!(root.children.len(), 1);

        let pi = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(pi.kind().unwrap(), ElementKind::new("tur_pointer_interact"));
        assert_eq!(pi.children.len(), 1);

        let container = tree
            .get_element(ElementNodeId::new(pi.children[0].as_u64()))
            .unwrap();
        assert_eq!(container.kind().unwrap(), ElementKind::new("tur_container"));

        (pi.id, container.id)
    };

    app.render();
    let rt = app.element_tree();

    let pi_node = rt.get_element(pi_id).unwrap();
    assert_eq!(pi_node.computed_layout.size.width, 100.0);
    assert_eq!(pi_node.computed_layout.size.height, 50.0);

    let container_node = rt.get_element(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 100.0);
    assert_eq!(container_node.computed_layout.size.height, 50.0);
    assert_eq!(container_node.computed_layout.offset.x, 0.0);
    assert_eq!(container_node.computed_layout.offset.y, 0.0);
}

#[test]
fn pointer_interact_passes_through_in_column() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-interact-in-column").unwrap();

    let (pi1_id, pi2_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(col.children.len(), 2);

        let pi1 = tree
            .get_element(ElementNodeId::new(col.children[0].as_u64()))
            .unwrap();
        let pi2 = tree
            .get_element(ElementNodeId::new(col.children[1].as_u64()))
            .unwrap();
        assert_eq!(
            pi1.kind().unwrap(),
            ElementKind::new("tur_pointer_interact")
        );
        assert_eq!(
            pi2.kind().unwrap(),
            ElementKind::new("tur_pointer_interact")
        );

        let sb1 = tree
            .get_element(ElementNodeId::new(pi1.children[0].as_u64()))
            .unwrap();
        let sb2 = tree
            .get_element(ElementNodeId::new(pi2.children[0].as_u64()))
            .unwrap();

        (pi1.id, pi2.id, sb1.id, sb2.id)
    };

    app.render();
    let rt = app.element_tree();

    let pi1_node = rt.get_element(pi1_id).unwrap();
    assert_eq!(pi1_node.computed_layout.size.width, 80.0);
    assert_eq!(pi1_node.computed_layout.size.height, 40.0);
    assert_eq!(pi1_node.computed_layout.offset.y, 0.0);

    let sb1_node = rt.get_element(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let pi2_node = rt.get_element(pi2_id).unwrap();
    assert_eq!(pi2_node.computed_layout.size.width, 60.0);
    assert_eq!(pi2_node.computed_layout.size.height, 30.0);
    assert_eq!(pi2_node.computed_layout.offset.y, 40.0);

    let sb2_node = rt.get_element(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.offset.y, 0.0);
}
