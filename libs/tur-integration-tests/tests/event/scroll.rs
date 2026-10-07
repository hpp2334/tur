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
// Sidebar-shape pin — the playground's root sidebar nesting (Container →
// Column[header, Expanded[ScrollView[Column[rows]]]]) must scroll from a
// real `ShellEvent::Wheel` pushed by the platform: graded deltas
// accumulate, clamp at maxScrollExtent (the "clamps after one event"
// observation is boundary physics, not a bug), and a wheel outside the
// sidebar does nothing.
// ===========================================================================

#[test]
fn wheel_scrolls_the_playground_sidebar_shape() {
    let mut app = TurTestApp::new(400.0, 800.0).unwrap();
    app.load_rut_module(
        r#"
use tur::{ AXIS_VERTICAL, CROSS_ALIGN_STRETCH, mount };
use tur_kit::{ SourceF64,  Column, Container, Expanded, ScrollView, Text };


entry fn start() -> u64 {
    let header = Container().padding(14.0)
        .child(Text().text("CASES").font_size(10.0).build());
    let rows = Column().cross_alignment(CROSS_ALIGN_STRETCH);
    // 19 fixed-height rows — the sidebar's overflow content (939px total
    // against a 754px scroll viewport in the 800-tall fixture window).
    let rows = rows.child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build())
        .child(Container().width_height(200.0, 47.0).color(0x0F172AFFu64).build());
    let sidebar = Container().width(200.0)
        .child(Column().cross_alignment(CROSS_ALIGN_STRETCH)
            .child(header.build())
            .child(Expanded().flex(1.0)
                .child(ScrollView().axis(AXIS_VERTICAL)
                    .child(rows.build())
                    .query_key("sidebar-scroll")
                    .build())
                .build())
            .build());
    mount(sidebar.build());
    return 0;
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let sv_id = ElementNodeId::new(app.query_element(&["sidebar-scroll"]).unwrap().as_u64());

    // First notch (120px) — graded scroll from zero.
    app.wheel(0.0, 120.0, 100.0, 400.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 120.0);
    })
    .unwrap();

    // Second notch lands on the boundary; a third clamps exactly at
    // maxScrollExtent (content 893 − viewport ≈754 → the "clamps after one
    // event" observation is boundary physics, not a bug).
    app.wheel(0.0, 120.0, 100.0, 400.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.wheel(0.0, 120.0, 100.0, 400.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert!(
            sv.max_scroll_extent() >= 120.0,
            "fixture must overflow: max={}",
            sv.max_scroll_extent()
        );
        let max = sv.max_scroll_extent();
        assert!(
            (sv.scroll_offset() - max).abs() < 0.001,
            "offset must clamp at maxScrollExtent: offset={}, max={max}",
            sv.scroll_offset()
        );
    })
    .unwrap();

    // A wheel outside the sidebar (editor-pane side) does nothing.
    app.wheel(0.0, 120.0, 300.0, 400.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), sv.max_scroll_extent());
    })
    .unwrap();
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
    app.load_rut_module(
        r#"
use tur::{ ctx_bridge, AXIS_VERTICAL, mount };
use tur_kit::{ Container, MutationCtx, Readable, ScrollView, source_f64 };


entry fn start() -> u64 {
    let height: Readable<f64> = source_f64(900.0);

    let b = Container().width_height(10.0, 10.0).color(0x204080FFu64).height_bound(height);

    let mut scroller = ScrollView().axis(AXIS_VERTICAL).child(b.build()).query_key("sv").build();
    let scroller = scroller;
    mount(scroller);
    return height.atom_id();
}

entry fn shrink(atom: u64, _b: f64) {
    let height = SourceF64.of(ctx_bridge(), atom, false);
    MutationCtx.over(ctx_bridge()).set<f64>(height, 200.0);
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let sv_id = ElementNodeId::new(app.query_element(&["sv"]).unwrap().as_u64());
    let height_atom = app.rut_start_answer();

    // Content 900 in a 300-tall viewport → maxScrollExtent 600. Scroll to
    // 300 with a wheel event.
    app.wheel(0.0, 300.0, 200.0, 150.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(sv.scroll_offset(), 300.0);
        assert_eq!(sv.max_scroll_extent(), 600.0);
    })
    .unwrap();

    // Shrink the content to 200 → maxScrollExtent collapses to 0 → the
    // offset must clamp during layout, not stay stale.
    app.call_rut_entry("shrink", height_atom, 0.0).unwrap();
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

    // The scroll METRICS the controller caches are synced by layout (the
    // element's own state) — read through the element probe.
    app.with_element(sv_id, |e| {
        let sv = e.cast::<ScrollViewElement>().unwrap();
        assert_eq!(
            sv.max_scroll_extent(),
            0.0,
            "metrics must reflect the clamp"
        );
    })
    .unwrap();
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
