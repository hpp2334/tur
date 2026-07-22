use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn column_basic_vertical_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    let (col_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        assert_eq!(
            root.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_root")
        );
        assert_eq!(root.children.len(), 1);

        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            col.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(col.children.len(), 2);

        let sb1 = tree.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
        assert_eq!(
            sb1.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        let sb2 = tree.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();
        assert_eq!(
            sb2.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        (col.id, sb1.id, sb2.id)
    };

    app.render();
    let rt = app.element_tree();

    let sb1_node = rt.get_element(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.size.height, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let sb2_node = rt.get_element(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.size.height, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 50.0);

    let col_node = rt.get_element(col_id).unwrap();
    assert_eq!(col_node.computed_layout.size.height, 600.0);
}

#[test]
fn column_main_alignment_end() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-main-end").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(col.children.len(), 2);
        (col.children[0], col.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1_node = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1_node.computed_layout.size.height, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 520.0);

    let sb2_node = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2_node.computed_layout.size.height, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 570.0);
}

#[test]
fn column_cross_alignment_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-cross-start").unwrap();

    let sb1_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        col.children[0]
    };

    app.render();
    let rt = app.element_tree();
    let sb1_node = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
}

#[test]
fn column_nested_children_do_not_overlap() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-nested").unwrap();

    let (_outer_col_id, sb1_id, inner_col_id, sb3_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let outer_col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(outer_col.children.len(), 3);
        let inner_col = tree.get_element(ElementNodeId::new(outer_col.children[1].as_u64())).unwrap();
        (
            outer_col.id,
            outer_col.children[0],
            inner_col.id,
            outer_col.children[2],
        )
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.height, 50.0);
    assert_eq!(sb1.computed_layout.offset.y, 0.0);

    let inner_col = rt.get_element(inner_col_id).unwrap();
    assert_eq!(
        inner_col.computed_layout.offset.y, 50.0,
        "inner column should start after first child"
    );
    assert_eq!(
        inner_col.computed_layout.size.height, 30.0,
        "inner column with mainAxisSize=Min should size to content"
    );

    let sb3 = rt.get_element(ElementNodeId::new(sb3_id.as_u64())).unwrap();
    assert_eq!(
        sb3.computed_layout.size.height, 40.0,
        "third child should have non-zero height (not starved by inner column)"
    );
    assert_eq!(
        sb3.computed_layout.offset.y, 80.0,
        "third child should be positioned after inner column (50 + 30)"
    );
}

#[test]
fn column_overflow_children_keep_natural_height() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-overflow").unwrap();

    let (col_id, c0, c1, c2) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(col.children.len(), 3);
        (col.id, col.children[0], col.children[1], col.children[2])
    };

    app.render();
    let rt = app.element_tree();

    let col = rt.get_element(col_id).unwrap();
    assert_eq!(
        col.computed_layout.size.height, 600.0,
        "column clamped to viewport height"
    );

    for (i, &child_id) in [c0, c1, c2].iter().enumerate() {
        let child = rt.get_element(ElementNodeId::new(child_id.as_u64())).unwrap();
        assert_eq!(
            child.computed_layout.size.height, 300.0,
            "child {i} should keep its natural 300px height, not be squished"
        );
        assert_eq!(
            child.computed_layout.offset.y, (i * 300) as f64,
            "child {i} offset should be {i}*300"
        );
    }
}
