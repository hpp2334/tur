use tur_integration_tests::TurTestApp;

#[test]
fn row_stretch_non_expanded_with_expanded_sibling() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-cross-stretch-no-height").unwrap();

    let (sidebar_id, divider_id, expanded_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let column = tree.get(root.children[0]).unwrap();
        let row = tree.get(column.children[0]).unwrap();
        assert_eq!(row.children.len(), 3, "row should have 3 children");
        (row.children[0], row.children[1], row.children[2])
    };

    app.render();
    let rt = app.element_tree();

    // All three children should be 600 tall (the row's height).
    let sidebar = rt.get(sidebar_id).unwrap();
    assert_eq!(sidebar.computed_layout.size.width, 100.0);
    assert_eq!(
        sidebar.computed_layout.size.height, 600.0,
        "sidebar (Container with width=100) should stretch to row height"
    );

    let divider = rt.get(divider_id).unwrap();
    assert_eq!(divider.computed_layout.size.width, 8.0);
    assert_eq!(
        divider.computed_layout.size.height, 600.0,
        "divider (Container with width=8) should stretch to row height"
    );

    let expanded = rt.get(expanded_id).unwrap();
    assert_eq!(expanded.computed_layout.size.width, 292.0); // 400 - 100 - 8
    assert_eq!(expanded.computed_layout.size.height, 600.0);
}
