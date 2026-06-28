use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::elements::ScrollViewElement;
use tur_integration_tests::TurTestApp;

#[test]
fn scroll_view_viewport_constrained() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let (sv_id, col_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        assert_eq!(
            sv.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_scroll_view")
        );
        assert_eq!(sv.children.len(), 1);

        let col = tree.get_element(ElementNodeId::new(sv.children[0].as_u64())).unwrap();
        assert_eq!(
            col.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(col.children.len(), 3);

        (sv.id, col.id)
    };

    app.render();
    let rt = app.element_tree();

    let sv_node = rt.get_element(ElementNodeId::new(sv_id.as_u64())).unwrap();
    assert_eq!(sv_node.computed_layout.size.width, 400.0);
    assert_eq!(sv_node.computed_layout.size.height, 300.0);

    let col_node = rt.get_element(ElementNodeId::new(col_id.as_u64())).unwrap();
    assert_eq!(col_node.computed_layout.size.height, 600.0);
}

#[test]
fn scroll_view_child_offset_zero() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let col_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        sv.children[0]
    };

    app.render();
    let rt = app.element_tree();

    let col_node = rt.get_element(ElementNodeId::new(col_id.as_u64())).unwrap();
    assert_eq!(col_node.computed_layout.offset.y, 0.0);
    assert_eq!(col_node.computed_layout.offset.x, 0.0);
}

#[test]
fn scroll_view_child_offset_with_prop() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-offset").unwrap();

    let col_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        sv.children[0]
    };

    app.render();
    let rt = app.element_tree();

    let col_node = rt.get_element(ElementNodeId::new(col_id.as_u64())).unwrap();
    assert_eq!(col_node.computed_layout.offset.y, -100.0);
}

#[test]
fn scroll_view_content_and_viewport_size() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let sv_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        root.children[0]
    };

    app.render();

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.viewport_size().width, 400.0);
        assert_eq!(sv.viewport_size().height, 300.0);
        assert_eq!(sv.content_size().height, 600.0);
    });
}

#[test]
fn scroll_view_children_stacked_correctly() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let (sb1_id, sb2_id, sb3_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        let col = tree.get_element(ElementNodeId::new(sv.children[0].as_u64())).unwrap();
        (col.children[0], col.children[1], col.children[2])
    };

    app.render();
    let rt = app.element_tree();

    let sb1 = rt.get_element(ElementNodeId::new(sb1_id.as_u64())).unwrap();
    assert_eq!(sb1.computed_layout.size.height, 200.0);
    assert_eq!(sb1.computed_layout.offset.y, 0.0);

    let sb2 = rt.get_element(ElementNodeId::new(sb2_id.as_u64())).unwrap();
    assert_eq!(sb2.computed_layout.offset.y, 200.0);

    let sb3 = rt.get_element(ElementNodeId::new(sb3_id.as_u64())).unwrap();
    assert_eq!(sb3.computed_layout.offset.y, 400.0);
}
