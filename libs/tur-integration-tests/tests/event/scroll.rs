use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_engine::elements::ScrollViewElement;
use tur_integration_tests::TurTestApp;

fn setup_basic() -> (TurTestApp, NodeId, NodeId) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let (sv_id, col_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        (sv.id.into(), sv.children[0])
    };

    app.render();
    (app, sv_id, col_id)
}

#[test]
fn wheel_updates_scroll_offset() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 50.0, 200.0, 150.0);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 50.0);
    });
}

#[test]
fn wheel_clamps_at_zero() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, -50.0, 200.0, 150.0);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 0.0);
    });
}

#[test]
fn wheel_clamps_at_max_scroll() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 9999.0, 200.0, 150.0);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 300.0);
    });
}

#[test]
fn wheel_accumulates_offset() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 100.0, 200.0, 150.0);
    app.wheel(0.0, 50.0, 200.0, 150.0);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 150.0);
    });
}

#[test]
fn wheel_updates_child_position() {
    let (mut app, _sv_id, col_id) = setup_basic();

    app.wheel(0.0, 100.0, 200.0, 150.0);

    let rt = app.element_tree();
    let col_node = rt.get_element(ElementNodeId::new(col_id.as_u64())).unwrap();
    assert_eq!(col_node.computed_layout.offset.y, -100.0);
}

#[test]
fn wheel_miss_does_nothing() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 50.0, 999.0, 999.0);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 0.0);
    });
}

#[test]
fn wheel_chains_to_parent_at_boundary() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("scroll-view-nested").unwrap();

    let (outer_id, inner_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        let outer = tree.get_element(ElementNodeId::new(row.children[1].as_u64())).unwrap();
        let col = tree.get_element(ElementNodeId::new(outer.children[0].as_u64())).unwrap();
        let wrapper = tree.get_element(ElementNodeId::new(col.children[1].as_u64())).unwrap();
        let inner = tree.get_element(ElementNodeId::new(wrapper.children[0].as_u64())).unwrap();
        (outer.id.into(), inner.id.into())
    };

    app.render();

    app.wheel(0.0, 9999.0, 300.0, 200.0);

    app.with_element(inner_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        let max_inner = sv.content_size().height - sv.viewport_size().height;
        assert!(
            (sv.scroll_offset() - max_inner).abs() < 1.0,
            "inner should be at max scroll: offset={}, max={}",
            sv.scroll_offset(),
            max_inner
        );
    });

    let inner_max = app.with_element(inner_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        sv.scroll_offset()
    }).unwrap();

    app.wheel(0.0, 100.0, 300.0, 200.0);

    app.with_element(outer_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert!(
            sv.scroll_offset() > 0.0,
            "outer should have scrolled because inner was at boundary"
        );
    });

    app.with_element(inner_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(
            sv.scroll_offset(), inner_max,
            "inner should still be at max"
        );
    });
}
