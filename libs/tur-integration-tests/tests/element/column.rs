use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn column_basic_vertical_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    let (col_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        assert_eq!(
            root.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(root.children.len(), 1);

        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(
            col.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(col.children.len(), 2);

        let sb1 = tree.get(col.children[0]).unwrap();
        assert_eq!(
            sb1.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        let sb2 = tree.get(col.children[1]).unwrap();
        assert_eq!(
            sb2.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        (col.id, sb1.id, sb2.id)
    };

    app.render();
    let rt = app.element_tree();

    let sb1_node = rt.get(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.size.height, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let sb2_node = rt.get(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.size.height, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 50.0);

    let col_node = rt.get(col_id).unwrap();
    assert_eq!(col_node.computed_layout.size.height, 80.0);
}

#[test]
fn column_main_alignment_end() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-main-end").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(col.children.len(), 2);
        (col.children[0], col.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1_node = rt.get(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.size.height, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 520.0);

    let sb2_node = rt.get(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.size.height, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.y, 570.0);
}

#[test]
fn column_cross_alignment_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-cross-start").unwrap();

    let sb1_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        col.children[0]
    };

    app.render();
    let rt = app.element_tree();
    let sb1_node = rt.get(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
}
