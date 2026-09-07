use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn positioned_with_left_top() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-basic").unwrap();

    let pos_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let stack = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        assert_eq!(stack.kind().unwrap(), ElementKind::new("tur_stack"));
        assert_eq!(stack.children.len(), 1);

        let positioned = tree
            .get_element(ElementNodeId::new(stack.children[0].as_u64()))
            .unwrap();
        assert_eq!(
            positioned.kind().unwrap(),
            ElementKind::new("tur_positioned")
        );
        assert_eq!(positioned.children.len(), 1);

        let sb = tree
            .get_element(ElementNodeId::new(positioned.children[0].as_u64()))
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

/// The `Positioned` node wrapping the queried child (queryKey lives on the
/// child — `Positioned` itself has no queryKey prop).
fn positioned_parent_of(app: &TurTestApp, key: &[&str]) -> ElementNodeId {
    let child = app.query_element(key).unwrap();
    let tree = app.element_tree();
    let node = tree
        .get_element(ElementNodeId::new(child.as_u64()))
        .unwrap();
    let parent = node.parent.unwrap();
    ElementNodeId::new(parent.as_u64())
}

/// `right`/`bottom` anchors place from the stack's far edges (Flutter:
/// `x = size.width - right - child.width`). The stack is sized 200x300 by
/// its non-positioned child, so the 60x24 pill at right:8/bottom:8 must sit
/// at (132, 268) — not at the top-left corner.
#[test]
fn positioned_right_bottom_anchors_from_stack_edges() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-right-bottom").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let pos_id = positioned_parent_of(&app, &["rb-pill"]);
    let rt = app.element_tree();
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 200.0 - 8.0 - 60.0);
    assert_eq!(pos_node.computed_layout.offset.y, 300.0 - 8.0 - 24.0);
}

/// An opposing edge pair (`left`+`right`, no explicit width) sizes the child
/// to the STACK's width — 200 - 10 - 10 = 180 — not the incoming constraint
/// max (400), and the pair anchors from `left`.
#[test]
fn positioned_edge_pair_uses_stack_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-right-bottom").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let pos_id = positioned_parent_of(&app, &["lr-pair"]);
    let rt = app.element_tree();
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.size.width, 180.0);
    assert_eq!(pos_node.computed_layout.offset.x, 10.0);
}

/// A Stack with ONLY positioned children sizes itself to the biggest size
/// its constraints allow (Flutter `RenderStack`: `size = constraints.biggest`)
/// — the reference box that right/bottom anchors resolve against.
#[test]
fn positioned_only_stack_sizes_to_constraints_biggest() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("positioned-only-stack").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let stack_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let expanded = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        let stack = tree
            .get_element(ElementNodeId::new(expanded.children[0].as_u64()))
            .unwrap();
        assert_eq!(stack.kind().unwrap(), ElementKind::new("tur_stack"));
        stack.id
    };

    let rt = app.element_tree();
    let stack = rt.get_element(stack_id).unwrap();
    assert_eq!(stack.computed_layout.size.width, 400.0);
    assert_eq!(stack.computed_layout.size.height, 600.0);

    let pos_id = positioned_parent_of(&app, &["dot"]);
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 5.0);
    assert_eq!(pos_node.computed_layout.offset.y, 5.0);
}
