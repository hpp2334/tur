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

// ---------------------------------------------------------------------------
// pos_left_bound / pos_top_bound — the reactive anchors (the Phase-9
// jigsaw hand-off: drag-to-move writes the live atom; the Positioned
// re-resolves its offset). Unlike `pos_left`'s authoring idiom (0 =
// absent — the `pos_left` row's doc), a BOUND anchor is present verbatim:
// 0 is a real coordinate (a piece parked at the origin).
// ---------------------------------------------------------------------------

const BOUND_ANCHORS_RUT: &str = r#"
use tur::{ mount, rs_set_f64, rs_source_f64, stf_put, stf_take };
use tur_kit::{ Container, Positioned, Stack };

let K_X: u64 = 1;
let K_Y: u64 = 2;

entry fn start() {
    let x = rs_source_f64();
    let y = rs_source_f64();
    rs_set_f64(x, 30.0);
    rs_set_f64(y, 40.0);
    let stack = Stack().query_key("pos/board").child(
        Positioned().left_bound(x).top_bound(y)
            .child(Container().width_height(50.0, 50.0).query_key("pos/pill").build())
            .build(),
    ).build();
    stf_put(K_X, x as f64);
    stf_put(K_Y, y as f64);
    mount(stack);
}

entry fn probe_x(_a: u64, b: f64) {
    let x = stf_take(K_X) as u64;
    rs_set_f64(x, b);
    stf_put(K_X, x as f64);
}

entry fn probe_y(_a: u64, b: f64) {
    let y = stf_take(K_Y) as u64;
    rs_set_f64(y, b);
    stf_put(K_Y, y as f64);
}
"#;

#[test]
fn positioned_bound_anchors_follow_the_live_atom() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(BOUND_ANCHORS_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let pos_id = positioned_parent_of(&app, &["pos", "pill"]);
    let rt = app.element_tree();
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 30.0);
    assert_eq!(pos_node.computed_layout.offset.y, 40.0);

    // Drag-to-move shape: the write lands on the atom, the anchor
    // re-resolves — the Phase-9 jigsaw rail.
    app.call_rut_entry("probe_x", 0, 137.5).unwrap();
    app.call_rut_entry("probe_y", 0, 12.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 137.5);
    assert_eq!(pos_node.computed_layout.offset.y, 12.0);

    // A bound anchor's 0 is a REAL zero (not the authoring row's
    // 0-is-absent idiom): a piece parked at the origin stays put.
    app.call_rut_entry("probe_x", 0, 0.0).unwrap();
    app.call_rut_entry("probe_y", 0, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let rt = app.element_tree();
    let pos_node = rt.get_element(pos_id).unwrap();
    assert_eq!(pos_node.computed_layout.offset.x, 0.0);
    assert_eq!(pos_node.computed_layout.offset.y, 0.0);
}
