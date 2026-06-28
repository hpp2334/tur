use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

#[test]
fn row_main_alignment_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("row-basic").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.x, 0.0);
    assert_eq!(sb1.computed_layout.size.width, 50.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.x, 50.0);
    assert_eq!(sb2.computed_layout.size.width, 30.0);
}

#[test]
fn row_main_alignment_center() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-main-center").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.x, 160.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.x, 210.0);
}

#[test]
fn row_main_alignment_end() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-main-end").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.offset.x, 320.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.x, 370.0);
}

#[test]
fn row_cross_alignment_stretch() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-cross-stretch").unwrap();

    let (sb1_id, sb2_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.height, 600.0);
    assert_eq!(sb1.computed_layout.offset.y, 0.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.size.height, 600.0);
    assert_eq!(sb2.computed_layout.offset.y, 0.0);
}

#[test]
fn row_with_expanded() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("flex-row-expanded").unwrap();

    let (sb1_id, expanded_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.width, 50.0);
    assert_eq!(sb1.computed_layout.offset.x, 0.0);

    let expanded = rt.get_element(ElementNodeId::new(expanded_id.as_u64())).unwrap();
    assert_eq!(expanded.computed_layout.size.width, 350.0);
    assert_eq!(expanded.computed_layout.offset.x, 50.0);
}

#[test]
fn nested_sidebar_layout() {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_bundle("flex-nested-sidebar").unwrap();

    let (sidebar_id, content_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sidebar = rt.get_element(ElementNodeId::new(sidebar_id.as_u64())).unwrap();
    assert_eq!(sidebar.computed_layout.size.width, 200.0);
    assert_eq!(sidebar.computed_layout.offset.x, 0.0);
    assert_eq!(sidebar.computed_layout.size.height, 600.0);

    let content = rt.get_element(ElementNodeId::new(content_id.as_u64())).unwrap();
    assert_eq!(content.computed_layout.offset.x, 200.0);
    assert_eq!(content.computed_layout.size.height, 600.0);
}

#[test]
fn todolist_sidebar_at_left() {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_bundle("flex-todolist-sidebar").unwrap();

    let (sidebar_id, content_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (row.children[0], row.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let sidebar = rt.get_element(ElementNodeId::new(sidebar_id.as_u64())).unwrap();
    assert_eq!(
        sidebar.computed_layout.offset.x, 0.0,
        "sidebar should be at left (x=0)"
    );
    assert_eq!(sidebar.computed_layout.size.width, 200.0);

    let content = rt.get_element(ElementNodeId::new(content_id.as_u64())).unwrap();
    assert_eq!(
        content.computed_layout.offset.x, 200.0,
        "content should be at x=200"
    );
}
