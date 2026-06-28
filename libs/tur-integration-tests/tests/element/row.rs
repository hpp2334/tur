use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

#[test]
fn row_basic_horizontal_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-basic").unwrap();

    let (row_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        assert_eq!(
            root.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(root.children.len(), 1);

        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            row.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(row.children.len(), 2);

        let sb1 = tree.get_element(ElementNodeId::new(row.children[0].as_u64())).unwrap();
        let sb2 = tree.get_element(ElementNodeId::new(row.children[1].as_u64())).unwrap();
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

    let sb1_node = rt.get_element(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.size.width, 50.0);
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);

    let sb2_node = rt.get_element(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.size.width, 30.0);
    assert_eq!(sb2_node.computed_layout.offset.x, 50.0);

    let row_node = rt.get_element(row_id).unwrap();
    assert_eq!(row_node.computed_layout.size.width, 400.0);
}

#[test]
fn row_cross_center_in_tight_container() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-cross-center").unwrap();

    let (container_id, row_id, sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let container = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        let row = tree.get_element(ElementNodeId::new(container.children[0].as_u64())).unwrap();
        (
            container.id,
            row.id,
            row.children[0],
            row.children[1],
        )
    };

    app.render();
    let rt = app.element_tree();

    let container = rt.get_element(container_id).unwrap();
    assert_eq!(container.computed_layout.size.width, 200.0);
    assert_eq!(container.computed_layout.size.height, 36.0);

    let row = rt.get_element(row_id).unwrap();
    assert_eq!(row.computed_layout.size.height, 36.0,
        "Row inside Container(height=36) should be 36px tall (tight constraints)");

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.width, 20.0);
    assert_eq!(sb1.computed_layout.size.height, 20.0);
    assert_eq!(sb1.computed_layout.offset.y, 8.0,
        "20px child centered in 36px Row: (36-20)/2 = 8");

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.size.height, 10.0);
    assert_eq!(sb2.computed_layout.offset.y, 13.0,
        "10px child centered in 36px Row: (36-10)/2 = 13");
}

#[test]
fn row_cross_center_in_column_does_not_starve_siblings() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-cross-center-in-column").unwrap();

    let (row_id, sb1_id, sb2_id, sb3_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let col = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        let row = tree.get_element(ElementNodeId::new(col.children[0].as_u64())).unwrap();
        (
            row.id,
            row.children[0],
            row.children[1],
            col.children[2],
        )
    };

    app.render();
    let rt = app.element_tree();

    let row = rt.get_element(row_id).unwrap();
    assert_eq!(row.computed_layout.size.height, 20.0,
        "Row with MainAxisSize.Min should be tallest child height");

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.y, 0.0,
        "20px child in 20px Row: centered at y=0");

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.y, 5.0,
        "10px child centered in 20px Row: (20-10)/2 = 5");

    let sb3 = rt.get_element(ElementNodeId::new(sb3_id.as_u64())).unwrap();
    assert_eq!(sb3.computed_layout.size.height, 20.0);
    assert_eq!(sb3.computed_layout.offset.y, 50.0,
        "third child at y=50 (Row 20 + SizedBox 30), not starved");
}
