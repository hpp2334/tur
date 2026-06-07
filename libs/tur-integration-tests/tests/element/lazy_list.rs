use tur_engine::core::element::ElementKind;
use tur_engine::elements::LazyListElement;
use tur_integration_tests::TurTestApp;

#[test]
fn lazy_list_viewport_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-basic").unwrap();

    let ll_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        assert_eq!(
            root.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_flex")
        );
        assert_eq!(root.children.len(), 1);

        let ll = tree.get(root.children[0]).unwrap();
        assert_eq!(
            ll.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_lazy_list")
        );
        ll.id
    };

    app.render();
    let rt = app.element_tree();

    let ll_node = rt.get(ll_id).unwrap();
    assert_eq!(ll_node.computed_layout.size.width, 400.0);
    assert_eq!(ll_node.computed_layout.size.height, 600.0);
}

#[test]
fn lazy_list_children_positioned_by_index() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-basic").unwrap();

    let (child0_id, child1_id, child2_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let ll = tree.get(root.children[0]).unwrap();
        assert!(ll.children.len() >= 3, "should have at least 3 children");
        (ll.children[0], ll.children[1], ll.children[2])
    };

    app.render();
    let rt = app.element_tree();

    let c0 = rt.get(child0_id).unwrap();
    assert_eq!(c0.computed_layout.size.height, 50.0);
    assert_eq!(c0.computed_layout.offset.y, 0.0);

    let c1 = rt.get(child1_id).unwrap();
    assert_eq!(c1.computed_layout.size.height, 50.0);
    assert_eq!(c1.computed_layout.offset.y, 50.0);

    let c2 = rt.get(child2_id).unwrap();
    assert_eq!(c2.computed_layout.size.height, 50.0);
    assert_eq!(c2.computed_layout.offset.y, 100.0);
}

#[test]
fn lazy_list_children_tight_constraints() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-basic").unwrap();

    let child0_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let ll = tree.get(root.children[0]).unwrap();
        ll.children[0]
    };

    app.render();
    let rt = app.element_tree();

    let c0 = rt.get(child0_id).unwrap();
    assert_eq!(c0.computed_layout.size.width, 400.0);
    assert_eq!(c0.computed_layout.size.height, 50.0);
}

#[test]
fn lazy_list_element_properties() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-basic").unwrap();

    let ll_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        root.children[0]
    };

    app.render();

    app.with_element(ll_id, |e| {
        let ll = e.cast::<LazyListElement>().unwrap();
        assert_eq!(ll.item_count(), 20);
    });
}

#[test]
fn lazy_list_scroll_updates_position() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-scroll").unwrap();

    app.render();

    app.wheel(0.0, 200.0, 200.0, 300.0);
    app.render();

    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let ll = tree.get(root.children[0]).unwrap();

    let c0 = tree.get(ll.children[0]).unwrap();
    assert_eq!(
        c0.computed_layout.offset.y, -200.0,
        "first child should shift up by 200px after scroll"
    );
}

#[test]
fn lazy_list_row_horizontal_layout() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("lazy-list-row").unwrap();

    let (child0_id, child1_id) = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        let ll = tree.get(root.children[0]).unwrap();
        assert!(ll.children.len() >= 2);
        (ll.children[0], ll.children[1])
    };

    app.render();
    let rt = app.element_tree();

    let c0 = rt.get(child0_id).unwrap();
    assert_eq!(c0.computed_layout.size.width, 80.0);
    assert_eq!(c0.computed_layout.size.height, 300.0);
    assert_eq!(c0.computed_layout.offset.x, 0.0);

    let c1 = rt.get(child1_id).unwrap();
    assert_eq!(c1.computed_layout.size.width, 80.0);
    assert_eq!(c1.computed_layout.offset.x, 80.0);
}

#[test]
fn lazy_list_scroll_clamps_at_content_end() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-scroll").unwrap();

    let ll_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        root.children[0]
    };

    app.render();

    app.wheel(0.0, 50000.0, 200.0, 300.0);
    app.render();

    app.with_element(ll_id, |e| {
        let ll = e.cast::<LazyListElement>().unwrap();
        let max_scroll = 100.0 * 50.0 - 600.0;
        let offset = ll.scroll_offset();
        assert!(
            offset <= max_scroll + 0.1,
            "scroll should be clamped at content end: {} > {}",
            offset,
            max_scroll
        );
        assert!(
            offset > 0.0,
            "scroll should have moved from 0, got {}",
            offset
        );
    });
}
