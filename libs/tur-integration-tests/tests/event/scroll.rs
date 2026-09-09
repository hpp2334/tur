use tur_engine::builtin_plugins::scroll::ScrollViewElement;
use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_integration_tests::TurTestApp;

fn setup_basic() -> (TurTestApp, ElementNodeId, NodeId) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let (sv_id, col_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (sv.id, sv.children[0])
    };

    app.wait_for_timeout(std::time::Duration::ZERO);
    (app, sv_id, col_id)
}

#[test]
fn wheel_updates_scroll_offset() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 50.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 50.0);
    });
}

#[test]
fn wheel_clamps_at_zero() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, -50.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 0.0);
    });
}

#[test]
fn wheel_clamps_at_max_scroll() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 9999.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 300.0);
    });
}

#[test]
fn wheel_accumulates_offset() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 100.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.wheel(0.0, 50.0, 200.0, 150.0);
    // `wheel` is fire-and-forget: the platform event only flushes on a
    // driven frame. This wait was missing — the assert below used to read
    // the pre-second-wheel offset (100), which the old swallowed-panic
    // behavior hid (the failing assert never ran).
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 150.0);
    })
    .unwrap();
}

#[test]
fn wheel_updates_child_position() {
    let (mut app, _sv_id, col_id) = setup_basic();

    app.wheel(0.0, 100.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let rt = app.element_tree();
    let col_node = rt.get_element(ElementNodeId::new(col_id.as_u64())).unwrap();
    assert_eq!(col_node.computed_layout.offset.y, -100.0);
}

#[test]
fn wheel_miss_does_nothing() {
    let (mut app, sv_id, _) = setup_basic();

    app.wheel(0.0, 50.0, 999.0, 999.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 0.0);
    });
}

// ===========================================================================
// Content-shrink clamp — Flutter `applyContentDimensions` parity. When the
// content shrinks below the current scroll offset, layout must clamp the
// offset to the new maxScrollExtent (otherwise the viewport shows blank
// space past the content end), and the controller's onScroll must fire for
// the correction.
// ===========================================================================

#[test]
fn content_shrink_clamps_scroll_offset_to_new_max() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            mount, ScrollView, Container, createScrollController, mutate, source,
        } from "tur:std";
        const height$ = source(900.0);
        globalThis.__events = [];
        const ctrl = createScrollController({
            onScroll: mutate((_ctx, e) => { globalThis.__events.push(e.offset); }),
        });
        globalThis.__ctrl = ctrl;
        globalThis.__shrink = () => store.set(height$, 200.0);
        mount(ScrollView()
            .controller(ctrl)
            .queryKey(["sv"])
            .child(Container().width(400).height(height$).build())
            .build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let sv_id = ElementNodeId::new(app.query_element(&["sv"]).unwrap().as_u64());

    // Content 900 in a 300-tall viewport → maxScrollExtent 600. Jump to 300.
    app.eval_js("globalThis.__ctrl.jumpTo(300)");
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 300.0);
        assert_eq!(sv.max_scroll_extent(), 600.0);
    })
    .unwrap();

    // Shrink the content to 200 → maxScrollExtent collapses to 0 → the
    // offset must clamp during layout, not stay stale.
    app.eval_js("globalThis.__shrink()");
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.content_size().height, 200.0);
        assert_eq!(
            sv.scroll_offset(),
            0.0,
            "offset must clamp to the new maxScrollExtent (0) when content shrinks"
        );
    })
    .unwrap();

    // The content child must be translated back to the viewport origin —
    // a stale offset leaves the viewport blank past the content end.
    let child_y = {
        let tree = app.element_tree();
        let sv = tree.get_element(sv_id).unwrap();
        let child = tree
            .get_element(ElementNodeId::new(sv.children[0].as_u64()))
            .unwrap();
        child.computed_layout.offset.y
    };
    assert_eq!(
        child_y, 0.0,
        "content must sit at viewport y=0 after the clamp"
    );

    // JS-visible controller metrics are synced, and onScroll fired for the
    // layout-driven correction (same frame, via the mutation queue).
    let ctrl_offset: f64 = app
        .eval_js("globalThis.__ctrl.offset")
        .trim()
        .parse()
        .unwrap();
    assert_eq!(ctrl_offset, 0.0, "controller.offset must reflect the clamp");
    let events = app.eval_js("JSON.stringify(globalThis.__events)");
    assert!(
        events.trim() == "[300,0]" || events.trim() == "[300, 0]",
        "onScroll must fire for the layout-driven clamp correction, got {events}"
    );
}

#[test]
fn wheel_chains_to_parent_at_boundary() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("scroll-view-nested").unwrap();

    let (outer_id, inner_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let row = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        // The outer ScrollView is wrapped in an Expanded (non-flex Row
        // children get unbounded width — Flutter parity), so navigate
        // through the flex-item wrapper.
        let expanded = tree
            .get_element(ElementNodeId::new(row.children[1].as_u64()))
            .unwrap();
        let outer = tree
            .get_element(ElementNodeId::new(expanded.children[0].as_u64()))
            .unwrap();
        let col = tree
            .get_element(ElementNodeId::new(outer.children[0].as_u64()))
            .unwrap();
        let wrapper = tree
            .get_element(ElementNodeId::new(col.children[1].as_u64()))
            .unwrap();
        let inner = tree
            .get_element(ElementNodeId::new(wrapper.children[0].as_u64()))
            .unwrap();
        (outer.id, inner.id)
    };

    app.wait_for_timeout(std::time::Duration::ZERO);

    app.wheel(0.0, 9999.0, 300.0, 200.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

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

    let inner_max = app
        .with_element(inner_id, |e| {
            let sv = e.cast::<ScrollViewElement>().unwrap();
            sv.scroll_offset()
        })
        .unwrap();

    app.wheel(0.0, 100.0, 300.0, 200.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.with_element(outer_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert!(
            sv.scroll_offset() > 0.0,
            "outer should have scrolled because inner was at boundary"
        );
    });

    app.with_element(inner_id, move |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(
            sv.scroll_offset(),
            inner_max,
            "inner should still be at max"
        );
    });
}
