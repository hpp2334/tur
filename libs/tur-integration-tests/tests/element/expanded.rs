use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn expanded_fills_remaining() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("expanded-basic").unwrap();

    let (expanded_id, _inner_sb_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            col.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(col.children.len(), 2);

        let sb = tree.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
        assert_eq!(
            sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        let expanded = tree.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();
        assert_eq!(
            expanded.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex_item")
        );
        assert_eq!(expanded.children.len(), 1);

        let inner_sb = tree.get_element(ElementNodeId::new(expanded.children[0].as_u64())).unwrap();
        assert_eq!(
            inner_sb.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        (expanded.id, inner_sb.id)
    };

    app.render();
    let rt = app.element_tree();
    let expanded_node = rt.get_element(ElementNodeId::new(expanded_id.as_u64())).unwrap();
    assert_eq!(expanded_node.computed_layout.size.height, 550.0);
    assert_eq!(expanded_node.computed_layout.offset.y, 50.0);
}

#[test]
fn expanded_multiple_share_evenly() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("expanded-multiple").unwrap();

    let (exp1_id, exp2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(col.children.len(), 2);

        let exp1 = tree.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
        let exp2 = tree.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();
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

    let exp1_node = rt.get_element(ElementNodeId::new(exp1_id.as_u64())).unwrap();
    assert_eq!(exp1_node.computed_layout.size.height, 300.0);
    assert_eq!(exp1_node.computed_layout.offset.y, 0.0);

    let exp2_node = rt.get_element(ElementNodeId::new(exp2_id.as_u64())).unwrap();
    assert_eq!(exp2_node.computed_layout.size.height, 300.0);
    assert_eq!(exp2_node.computed_layout.offset.y, 300.0);
}

#[test]
fn expanded_flex_weights_proportional() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("expanded-flex-weights").unwrap();

    let (exp1_id, exp2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(col.children.len(), 2);

        let exp1 = tree.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
        let exp2 = tree.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();
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

    // Two Expanded children with flex 2 and 1 should split 600px as 400 + 200.
    let exp1_node = rt.get_element(ElementNodeId::new(exp1_id.as_u64())).unwrap();
    assert_eq!(exp1_node.computed_layout.size.height, 400.0);
    assert_eq!(exp1_node.computed_layout.offset.y, 0.0);

    let exp2_node = rt.get_element(ElementNodeId::new(exp2_id.as_u64())).unwrap();
    assert_eq!(exp2_node.computed_layout.size.height, 200.0);
    assert_eq!(exp2_node.computed_layout.offset.y, 400.0);
}
