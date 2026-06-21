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
    // After the virtualization position-math fix, items are placed by their
    // logical index, not their slot in the children vector. With
    // itemExtent=50 and viewport=600, scrolling 200px means items [0, 11]
    // are mounted (200/50 = 4, so item 4 should be at viewport y=0).
    //
    // Previously this test asserted `c0.offset.y == -200`, which was the
    // buggy "cum starts at 0 regardless of first mounted index" output.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("lazy-list-scroll").unwrap();

    app.render();

    app.wheel(0.0, 200.0, 200.0, 300.0);
    app.render();

    let ll_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        root.children[0]
    };

    // First mounted index should be 4 (scroll/extent = 4).
    app.with_element(ll_id, |e| {
        let ll = e.cast::<LazyListElement>().unwrap();
        assert_eq!(ll.first_mounted_index(), Some(4),
            "first mounted item should be index 4 after scrolling 200px (extent=50)");
    });

    // Item 4 should be at viewport y=0 (content_pos = 4*50 = 200; viewport
    // y = 200 - 200 = 0). The previously-first item (index 0) is now
    // unmounted.
    let tree = app.element_tree();
    let ll = tree.get(ll_id).unwrap();
    let first_mounted_child = ll.children[0];
    let c0 = tree.get(first_mounted_child).unwrap();
    assert_eq!(
        c0.computed_layout.offset.y, 0.0,
        "first mounted child (index 4) should be at viewport y=0 after 200px scroll, got {}",
        c0.computed_layout.offset.y
    );

    // Item 5 should be at viewport y=50.
    let c1 = tree.get(ll.children[1]).unwrap();
    assert_eq!(
        c1.computed_layout.offset.y, 50.0,
        "second mounted child (index 5) should be at viewport y=50, got {}",
        c1.computed_layout.offset.y
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

#[test]
fn lazy_list_virtualizes_large_item_count() {
    // Build a LazyList inline with 10,000 fixed-extent items and verify
    // that only a small subset is actually mounted after layout + scroll.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_source(r#"
        const ctx = globalThis.__tur.__ctx;
        globalThis.__tur.render(ctx, globalThis.__tur.LazyList(ctx, {
            axis: 0,
            itemCount: 10000,
            itemExtent: 50,
            overscan: 2,
            builder: (i) => globalThis.__tur.Container(ctx, {
                height: 50,
                color: globalThis.__tur.createColor(ctx, 200, 200, 200, 255),
                children: [globalThis.__tur.Text(ctx, { text: "Item " + i })],
            }),
        }));
    "#).unwrap();
    app.render();

    let ll_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        root.children[0]
    };

    // After layout, the visible range should be roughly (viewport / extent)
    // + 2x overscan = 12 + 4 = 16 items — NOT 10,000.
    app.with_element(ll_id, |e| {
        let ll = e.cast::<LazyListElement>().unwrap();
        let built = ll.built_count();
        assert!(built < 50,
            "virtualized list should mount < 50 items, got {built}");
        assert!(built >= 12,
            "virtualized list should mount at least viewport/extent items, got {built}");
        assert_eq!(ll.item_count(), 10000,
            "declared item count should still be 10000");
    });

    // Scroll by 5000px = 100 items. The mounted set should shift to the
    // [~100, ~112] range but still be small.
    app.wheel(0.0, 5000.0, 200.0, 300.0);
    app.render();

    app.with_element(ll_id, |e| {
        let ll = e.cast::<LazyListElement>().unwrap();
        let built = ll.built_count();
        assert!(built < 50,
            "after scroll, virtualized list should still mount < 50 items, got {built}");
        // The first mounted index should be near 100.
        let first_idx = ll.first_mounted_index().unwrap_or(0);
        assert!(first_idx >= 95 && first_idx <= 105,
            "first mounted index should be near 100, got {first_idx}");
    });
}

// ===========================================================================
// Virtualization correctness — comprehensive integration tests covering
// scroll-driven mount/unmount, position math, and children ordering.
// ===========================================================================

/// Helper: load a virtualized 10,000-item list with fixed 56px extent.
fn setup_virtualized() -> (TurTestApp, tur_engine::core::element::ElementNodeId) {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_source(
        r#"
        const ctx = globalThis.__tur.__ctx;
        globalThis.__tur.render(ctx, globalThis.__tur.LazyList(ctx, {
            axis: 0,
            itemCount: 10000,
            itemExtent: 56,
            overscan: 2,
            queryKey: ["ll"],
            builder: (i) => globalThis.__tur.Container(ctx, {
                height: 56,
                color: globalThis.__tur.createColor(ctx, 200, 200, 200, 255),
                children: [globalThis.__tur.Text(ctx, { text: "Item " + i })],
            }),
        }));
        "#,
    )
    .unwrap();
    app.render();
    let id = app.query_element(&["ll"]).expect("queryKey ll not found");
    (app, id)
}

/// `with_element` helper that returns the closure's R directly (panics if
/// the lookup fails, which is fine for tests that just constructed the id).
fn with_ll<R>(app: &TurTestApp, id: tur_engine::core::element::ElementNodeId, f: impl FnOnce(&LazyListElement) -> R) -> R {
    app.with_element(id, |e| f(e.cast::<LazyListElement>().expect("not a LazyListElement")))
        .expect("element not found")
}

/// Sanity check: the initial visible range corresponds to the viewport.
/// With viewport=600 and extent=56, ~11 items fit (10.7), plus overscan=2
/// on each side = ~15 items. Should be far less than 10,000.
#[test]
fn virtualized_initial_mount_set_matches_viewport() {
    let (app, id) = setup_virtualized();

    let built = with_ll(&app, id, |ll| ll.built_count());
    let first = with_ll(&app, id, |ll| ll.first_mounted_index());
    let count = with_ll(&app, id, |ll| ll.item_count());

    assert!(built >= 11 && built <= 20,
        "initial mount set should be ~viewport/extent + 2*overscan (11..15), got {built}");
    assert_eq!(first, Some(0),
        "without scrolling, the first mounted item should be index 0");
    assert_eq!(count, 10000,
        "declared item count should remain 10000 regardless of mount set");
}

/// After scrolling, items that fall outside the visible range should be
/// unmounted. The total mounted count should stay roughly constant.
#[test]
fn virtualized_unmounts_offscreen_items_after_scroll() {
    let (mut app, id) = setup_virtualized();
    let initial_built = with_ll(&app, id, |ll| ll.built_count());

    // Scroll by 1000px = ~18 item heights. The visible window shifts by
    // 18 items but the total mounted count should stay close to initial.
    app.wheel(0.0, 1000.0, 200.0, 300.0);
    app.render();

    let after_built = with_ll(&app, id, |ll| ll.built_count());
    let first_idx = with_ll(&app, id, |ll| ll.first_mounted_index().unwrap());

    assert!(first_idx >= 14 && first_idx <= 20,
        "after scrolling 1000px (extent 56), first mounted index should be ~17, got {first_idx}");
    // Allow up to 4 difference for one-off overscan / rounding edges at the
    // top or bottom of the viewport.
    assert!((after_built as i64 - initial_built as i64).abs() <= 4,
        "mounted count should stay roughly constant after scroll (initial={initial_built}, after={after_built})");
}

/// Scroll far into the list and verify an item lands at viewport y≈0.
#[test]
fn virtualized_far_scroll_lands_on_correct_item() {
    let (mut app, id) = setup_virtualized();

    app.wheel(0.0, 5000.0, 200.0, 300.0);
    app.render();

    let first_idx = with_ll(&app, id, |ll| ll.first_mounted_index().unwrap());

    // First mounted item is start - overscan = floor(5000/56) - 2 = 87.
    // Allow a small tolerance for sub-pixel rounding.
    let expected_start: u64 = (5000.0_f64 / 56.0).floor() as u64;
    let expected_first = expected_start.saturating_sub(2);
    assert!(
        (first_idx as i64 - expected_first as i64).abs() <= 1,
        "first mounted index after 5000px scroll should be ~{expected_first}, got {first_idx}"
    );

    // At least one child should be near the top of the viewport (within one
    // item-extent of y=0). Items are spaced 56px apart, so the topmost
    // visible item is somewhere in y ∈ [-56, 0].
    let found = {
        let tree = app.element_tree();
        let ll_node = tree.get(id).unwrap();
        ll_node.children.iter().any(|&child_id| {
            let y = tree.get(child_id).unwrap().computed_layout.offset.y;
            y >= -56.0 && y <= 0.0
        })
    };
    assert!(found, "expected at least one child within one extent of viewport top after scroll");
}

/// Scrolling UP should keep the parent's children vector ordered by logical
/// index, even though newly-mounted items have smaller indices than
/// existing ones. Without the `link_child_before` fix, the new items would
/// land at the end of the vector and layout would scramble them.
#[test]
fn virtualized_scroll_up_keeps_children_ordered() {
    let (mut app, id) = setup_virtualized();

    // Scroll down a fair distance, then scroll back up.
    app.wheel(0.0, 2000.0, 200.0, 300.0);
    app.render();
    app.wheel(0.0, -1000.0, 200.0, 300.0);
    app.render();

    // Walk the parent's children vector, look up each child's logical index
    // via `visible_index_of`, and confirm the result is strictly increasing.
    let child_ids: Vec<_> = {
        let tree = app.element_tree();
        tree.get(id).unwrap().children.clone()
    };
    let mut prev: i64 = -1;
    for child_id in child_ids {
        let logical = with_ll(&app, id, |ll| ll.visible_index_of(child_id));
        let logical = logical.expect("every mounted child should have a logical index");
        assert!(
            (logical as i64) > prev,
            "children vector out of order: prev={prev}, current={logical} \
             (this means scrolling up scrambled the tree children)"
        );
        prev = logical as i64;
    }
}

/// Repeatedly scrolling down should keep children ordered at every step.
/// Catches regressions where the first scroll-down works but subsequent
/// ones scramble the order.
#[test]
fn virtualized_repeated_scroll_down_keeps_order() {
    let (mut app, id) = setup_virtualized();

    for _ in 0..5 {
        app.wheel(0.0, 500.0, 200.0, 300.0);
        app.render();

        let child_ids: Vec<_> = {
            let tree = app.element_tree();
            tree.get(id).unwrap().children.clone()
        };
        let mut prev: i64 = -1;
        for child_id in child_ids {
            let logical = with_ll(&app, id, |ll| ll.visible_index_of(child_id));
            let logical = logical.expect("child should have a logical index");
            assert!(
                (logical as i64) > prev,
                "after incremental scroll-down, children vector is out of order at logical={logical}"
            );
            prev = logical as i64;
        }
    }
}

/// Verify the position math for an arbitrary scroll offset: each mounted
/// child's viewport y should equal `logical_index * extent - scroll_offset`.
#[test]
fn virtualized_position_math_matches_content_index_formula() {
    let (mut app, id) = setup_virtualized();
    let extent = 56.0_f64;

    // Scroll by an odd amount so positions aren't trivially 0.
    let scroll_amount = 333.0_f64;
    app.wheel(0.0, scroll_amount, 200.0, 300.0);
    app.render();

    let child_ids: Vec<_> = {
        let tree = app.element_tree();
        tree.get(id).unwrap().children.clone()
    };
    for child_id in child_ids {
        let logical = with_ll(&app, id, |ll| ll.visible_index_of(child_id))
            .expect("logical index");
        let expected_y = (logical as f64) * extent - scroll_amount;
        let actual_y = app
            .element_tree()
            .get(child_id)
            .unwrap()
            .computed_layout
            .offset
            .y;
        assert!(
            (actual_y - expected_y).abs() < 0.5,
            "item {logical} should be at y={expected_y:.2}, got y={actual_y:.2}"
        );
    }
}

/// Mounted count after scroll should reflect viewport + 2*overscan, not the
/// full item count. This is the user-visible "is virtualization actually
/// working" sanity check.
#[test]
fn virtualized_reported_range_tracks_scroll() {
    let (mut app, id) = setup_virtualized();

    app.wheel(0.0, 1000.0, 200.0, 300.0);
    app.render();

    let first_mounted = with_ll(&app, id, |ll| ll.first_mounted_index().unwrap());
    let built = with_ll(&app, id, |ll| ll.built_count());

    // The first mounted index should be the visible-range start minus the
    // overscan (2). 1000 / 56 ≈ 17.86 → start = 17, minus overscan 2 = 15.
    // Allow ±1 for sub-pixel rounding.
    assert!(first_mounted >= 14 && first_mounted <= 18,
        "first mounted index after 1000px scroll should be ~15, got {first_mounted}");

    // And the mounted count should be the visible window + 2 * overscan.
    // Visible window = ceil(600/56) = 11. Total = 11 + 4 = 15, ±1.
    assert!(built >= 13 && built <= 17,
        "mounted count after scroll should be ~15, got {built}");
}

/// When the user scrolls past the end, the position should clamp at
/// `max_scroll_extent = item_count * extent - viewport`. No item should
/// appear at a negative viewport y.
#[test]
fn virtualized_scroll_past_end_clamps_and_no_negative_positions() {
    let (mut app, id) = setup_virtualized();

    // Way past the end.
    app.wheel(0.0, 999_999.0, 200.0, 300.0);
    app.render();

    let max_extent = 10000.0_f64 * 56.0 - 600.0;
    let scroll = with_ll(&app, id, |ll| ll.scroll_offset());
    assert!(scroll <= max_extent + 1.0,
        "scroll should clamp at max extent ({max_extent}), got {scroll}");

    // No mounted child should be absurdly off-screen. Allow up to 3 overscan
    // tolerances above/below the viewport to account for the boundary
    // between compute_visible_range and the scroll clamp.
    let overscan_tolerance = 3.0 * 56.0 + 1.0;
    let tree = app.element_tree();
    let ll_node = tree.get(id).unwrap();
    for &child_id in &ll_node.children {
        let y = tree.get(child_id).unwrap().computed_layout.offset.y;
        assert!(y > -overscan_tolerance,
            "child at id={child_id:?} has y={y}, should not be more than 3 extents above viewport");
        assert!(y < 600.0 + overscan_tolerance,
            "child at id={child_id:?} has y={y}, should be within viewport + overscan");
    }
}

/// Scroll back to 0 after going far down. All items should re-mount at the
/// top, first mounted index should be 0, and positions should match index*extent.
#[test]
fn virtualized_scroll_back_to_zero_resets_cleanly() {
    let (mut app, id) = setup_virtualized();

    app.wheel(0.0, 5000.0, 200.0, 300.0);
    app.render();
    app.wheel(0.0, -5000.0, 200.0, 300.0);
    app.render();

    let first_idx = with_ll(&app, id, |ll| ll.first_mounted_index());
    assert_eq!(first_idx, Some(0),
        "after scrolling back to 0, first mounted index should be 0, got {first_idx:?}");

    let (child_ids, ys): (Vec<_>, Vec<f64>) = {
        let tree = app.element_tree();
        let ll_node = tree.get(id).unwrap();
        (
            ll_node.children.clone(),
            ll_node
                .children
                .iter()
                .map(|&cid| tree.get(cid).unwrap().computed_layout.offset.y)
                .collect(),
        )
    };
    for (i, (_cid, y)) in child_ids.iter().zip(ys.iter()).enumerate() {
        let expected = i as f64 * 56.0;
        assert!((y - expected).abs() < 0.5,
            "after scroll-back-to-0, child at slot {i} should be at y={expected}, got y={y}");
    }
}

// ===========================================================================
// Regression: scroll-up must not duplicate children in the parent's children
// vector. `spec.build()` already calls `link_child` (append); calling
// `link_child_before` afterwards would insert the same id a second time,
// producing duplicates that crash layout / paint.
// ===========================================================================

/// Each entry in the parent's children vector must be unique — no duplicates.
#[test]
fn virtualized_no_duplicate_children_after_scroll_up() {
    let (mut app, id) = setup_virtualized();

    // Scroll down, then scroll back up — the upward leg mounts items at
    // lower indices than the existing ones, exercising the
    // `link_child_before` path.
    app.wheel(0.0, 2000.0, 200.0, 300.0);
    app.render();
    app.wheel(0.0, -1000.0, 200.0, 300.0);
    app.render();

    let child_ids: Vec<_> = {
        let tree = app.element_tree();
        tree.get(id).unwrap().children.clone()
    };
    let mut seen = std::collections::HashSet::new();
    for child_id in &child_ids {
        assert!(seen.insert(*child_id),
            "duplicate child id {:?} in parent's children vector (total len = {})",
            child_id, child_ids.len());
    }
}

/// The parent's children count must equal the LazyList's `built_count()` —
/// they should never diverge (which would indicate a bookkeeping bug like
/// double-add).
#[test]
fn virtualized_parent_children_count_matches_built_count() {
    let (mut app, id) = setup_virtualized();

    // Multiple scroll-up / scroll-down cycles to stress the remount path.
    for _ in 0..3 {
        app.wheel(0.0, 1500.0, 200.0, 300.0);
        app.render();
        app.wheel(0.0, -800.0, 200.0, 300.0);
        app.render();

        let parent_count = {
            let tree = app.element_tree();
            tree.get(id).unwrap().children.len()
        };
        let built = with_ll(&app, id, |ll| ll.built_count());
        assert_eq!(parent_count, built,
            "parent.children.len() ({parent_count}) should equal built_count ({built}) \
             — divergence indicates a mount/unmount bookkeeping bug");
    }
}

/// Repeated scroll-up that mounts lower-index items should not crash and
/// should leave every child in the children vector accessible (no orphan
/// ids that aren't in the tree). Catches the case where double-add creates
/// a stale entry that points to a since-destroyed node.
#[test]
fn virtualized_repeated_scroll_up_no_orphans_or_crash() {
    let (mut app, id) = setup_virtualized();

    // Cycle through many scroll-up/down sequences. Without the duplicate-
    // children fix this crashes within a few iterations because layout
    // dereferences a stale child id.
    for i in 0..10 {
        app.wheel(0.0, 1200.0, 200.0, 300.0);
        app.render();
        app.wheel(0.0, -600.0, 200.0, 300.0);
        app.render();

        // Walk the parent's children vector and dereference each id. With
        // the duplicate-add bug, this panics on a stale node lookup.
        let child_ids: Vec<_> = {
            let tree = app.element_tree();
            tree.get(id).unwrap().children.clone()
        };
        let tree = app.element_tree();
        for &child_id in &child_ids {
            let node = tree.get(child_id).unwrap_or_else(|| panic!(
                "iteration {i}: child id {child_id:?} in parent.children but not in tree \
                 (stale entry from double-add)"
            ));
            // Touch the computed layout to force the dereference all the
            // way through the node struct.
            let _ = node.computed_layout.offset.y;
        }
    }
}
