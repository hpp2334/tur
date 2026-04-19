use tur_integration_tests::TurTestApp;
use tur_render_tree::RenderNodeId;

#[test]
fn expanded_fills_remaining() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("expanded-basic").unwrap();

    let (expanded_id, _inner_sb_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(col.element.kind().as_str(), "tur_flex");
        assert_eq!(col.children.len(), 2);

        let sb = tree.get(col.children[0]).unwrap();
        assert_eq!(sb.element.kind().as_str(), "tur_container");

        let expanded = tree.get(col.children[1]).unwrap();
        assert_eq!(expanded.element.kind().as_str(), "tur_flex_item");
        assert_eq!(expanded.children.len(), 1);

        let inner_sb = tree.get(expanded.children[0]).unwrap();
        assert_eq!(inner_sb.element.kind().as_str(), "tur_container");

        (expanded.id.as_u64(), inner_sb.id.as_u64())
    };

    let rt = app.render_tree();
    let rt = rt.borrow();
    let expanded_node = rt.get(RenderNodeId::new(expanded_id)).unwrap();
    assert_eq!(expanded_node.computed_layout.size.height, 550.0);
    assert_eq!(expanded_node.computed_layout.offset.y, 50.0);
}

#[test]
fn expanded_multiple_share_evenly() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("expanded-multiple").unwrap();

    let (exp1_id, exp2_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(col.children.len(), 2);

        let exp1 = tree.get(col.children[0]).unwrap();
        let exp2 = tree.get(col.children[1]).unwrap();
        assert_eq!(exp1.element.kind().as_str(), "tur_flex_item");
        assert_eq!(exp2.element.kind().as_str(), "tur_flex_item");

        (exp1.id.as_u64(), exp2.id.as_u64())
    };

    let rt = app.render_tree();
    let rt = rt.borrow();

    let exp1_node = rt.get(RenderNodeId::new(exp1_id)).unwrap();
    assert_eq!(exp1_node.computed_layout.size.height, 300.0);
    assert_eq!(exp1_node.computed_layout.offset.y, 0.0);

    let exp2_node = rt.get(RenderNodeId::new(exp2_id)).unwrap();
    assert_eq!(exp2_node.computed_layout.size.height, 300.0);
    assert_eq!(exp2_node.computed_layout.offset.y, 300.0);
}
