use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn row_basic_horizontal_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-basic").unwrap();

    let (row_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        assert_eq!(
            root.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(root.children.len(), 1);

        let row = tree.get(root.children[0]).unwrap();
        assert_eq!(
            row.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(row.children.len(), 2);

        let sb1 = tree.get(row.children[0]).unwrap();
        let sb2 = tree.get(row.children[1]).unwrap();
        assert_eq!(
            sb1.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );
        assert_eq!(
            sb2.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        (row.id, sb1.id, sb2.id)
    };

    app.render();
    let rt = app.element_tree();

    let sb1_node = rt.get(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.size.width, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);

    let sb2_node = rt.get(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.size.width, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.x, 50.0);

    let row_node = rt.get(row_id).unwrap();
    assert_eq!(row_node.computed_layout.size.width, 80.0);
}
