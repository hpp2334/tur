use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn expanded_fills_remaining() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("expanded-basic").unwrap();

    let (expanded_id, _inner_sb_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(
            col.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(col.children.len(), 2);

        let sb = tree.get(col.children[0]).unwrap();
        assert_eq!(
            sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        let expanded = tree.get(col.children[1]).unwrap();
        assert_eq!(
            expanded.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex_item")
        );
        assert_eq!(expanded.children.len(), 1);

        let inner_sb = tree.get(expanded.children[0]).unwrap();
        assert_eq!(
            inner_sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        (expanded.id, inner_sb.id)
    };

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let expanded_node = rt.get(expanded_id).unwrap();
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
        assert_eq!(
            exp1.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex_item")
        );
        assert_eq!(
            exp2.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex_item")
        );

        (exp1.id, exp2.id)
    };

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();

    let exp1_node = rt.get(exp1_id).unwrap();
    assert_eq!(exp1_node.computed_layout.size.height, 300.0);
    assert_eq!(exp1_node.computed_layout.offset.y, 0.0);

    let exp2_node = rt.get(exp2_id).unwrap();
    assert_eq!(exp2_node.computed_layout.size.height, 300.0);
    assert_eq!(exp2_node.computed_layout.offset.y, 300.0);
}
