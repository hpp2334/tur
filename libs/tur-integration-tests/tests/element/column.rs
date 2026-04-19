use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn column_basic_vertical_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    let (col_id, sb1_id, sb2_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        assert_eq!(root.element.name(), "tur_flex");
        assert_eq!(root.children.len(), 1);

        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(col.element.name(), "tur_flex");
        assert_eq!(col.children.len(), 2);

        let sb1 = tree.get(col.children[0]).unwrap();
        assert_eq!(sb1.element.name(), "tur_container");

        let sb2 = tree.get(col.children[1]).unwrap();
        assert_eq!(sb2.element.name(), "tur_container");

        (col.id.as_u64(), sb1.id.as_u64(), sb2.id.as_u64())
    };

    let rt = app.render_tree();
    let rt = rt.borrow();

    let sb1_node = rt.get(RenderNodeId::new(sb1_id)).unwrap();
    assert_eq!(sb1_node.computed_layout.size.height, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let sb2_node = rt.get(RenderNodeId::new(sb2_id)).unwrap();
    assert_eq!(sb2_node.computed_layout.size.height, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 50.0);

    let col_node = rt.get(RenderNodeId::new(col_id)).unwrap();
    assert_eq!(col_node.computed_layout.size.height, 80.0);
}

#[test]
fn column_main_alignment_end() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-main-end").unwrap();

    let (sb1_id, sb2_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(col.children.len(), 2);
        (col.children[0].as_u64(), col.children[1].as_u64())
    };

    let rt = app.render_tree();
    let rt = rt.borrow();

    let sb1_node = rt.get(RenderNodeId::new(sb1_id)).unwrap();
    assert_eq!(sb1_node.computed_layout.size.height, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 520.0);

    let sb2_node = rt.get(RenderNodeId::new(sb2_id)).unwrap();
    assert_eq!(sb2_node.computed_layout.size.height, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 570.0);
}

#[test]
fn column_cross_alignment_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-cross-start").unwrap();

    let sb1_id = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        col.children[0].as_u64()
    };

    let rt = app.render_tree();
    let rt = rt.borrow();
    let sb1_node = rt.get(RenderNodeId::new(sb1_id)).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
}
