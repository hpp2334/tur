use tur_engine::builtin_plugins::lazy_container::LazyGridElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// Build a 10,000-item virtualized grid inline: 400x600 viewport,
/// maxCrossAxisExtent 100 → 4 columns of 100x100 cells, stride 100.
fn setup_virtualized() -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(
        r#"
use tur::{ mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ Container, Expanded, LazyGrid };


fn cell(i: u64) -> opaque {
    let b = Container().width_height(100.0, 100.0).color(0xC8C8C8FFu64);
    return b.build();
}

entry fn start() -> u64 {
    let count = rs_source_f64();
    rs_set_f64(count, 10000.0);
    let mut lg = LazyGrid().item_builder(cell).count(count).max_cross(100.0).aspect(1.0).query_key("lg").build();
    let lg = lg;
    let root = Expanded().flex(1.0).child(lg).build();
    mount(root);
    return count;
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = app.query_element(&["lg"]).expect("queryKey 'lg' not found");
    (app, ElementNodeId::new(id.as_u64()))
}

fn with_lg<R: Send + 'static>(
    app: &TurTestApp,
    id: ElementNodeId,
    f: impl FnOnce(&LazyGridElement) -> R + Send + 'static,
) -> R {
    app.with_element(id, |e| {
        f(e.cast::<LazyGridElement>().expect("not a LazyGridElement"))
    })
    .expect("element not found")
}

#[test]
fn lazy_grid_mounts_as_tur_lazy_grid() {
    let (app, id) = setup_virtualized();
    let tree = app.element_tree();
    let lg = tree.get_element(id).unwrap();
    assert_eq!(lg.kind().unwrap(), ElementKind::new("tur_lazy_grid"));
}

/// Only the viewport + overscan cells mount, not all 10,000.
/// 4 cols × ~7 visible rows (600/100) + 2×overscan rows = ~9 rows → ~36 cells.
#[test]
fn lazy_grid_virtualizes_large_item_count() {
    let (app, id) = setup_virtualized();

    let built = with_lg(&app, id, |lg| lg.built_count());
    assert!(
        built < 50,
        "virtualized grid should mount < 50 cells, got {built}"
    );
    assert!(
        built >= 28,
        "virtualized grid should mount at least ~viewport rows * cols, got {built}"
    );
}

/// Each mounted cell's viewport position must match the analytic formula:
///   row = index / cols, col = index % cols
///   x = col * cell_cross, y = row * stride - scroll_offset
#[test]
fn lazy_grid_position_math_matches_formula() {
    let (mut app, id) = setup_virtualized();
    let cell_cross = 100.0_f64;
    let stride = 100.0_f64;
    let cols = 4_usize;
    let scroll = 333.0_f64;

    app.wheel(0.0, scroll, 200.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let child_ids: Vec<_> = {
        let tree = app.element_tree();
        tree.get_element(id).unwrap().children.to_vec()
    };
    assert!(!child_ids.is_empty());
    for child_id in &child_ids {
        let child_id = *child_id;
        let logical = with_lg(&app, id, move |lg| lg.visible_index_of(child_id))
            .expect("every mounted cell should have a logical index");
        let row = logical as usize / cols;
        let col = logical as usize % cols;
        let expected_x = col as f64 * cell_cross;
        let expected_y = row as f64 * stride - scroll;
        let tree = app.element_tree();
        let node = tree
            .get_element(ElementNodeId::new(child_id.as_u64()))
            .unwrap();
        assert!(
            (node.computed_layout.offset.x - expected_x).abs() < 0.5,
            "cell {logical} x should be {expected_x}, got {}",
            node.computed_layout.offset.x
        );
        assert!(
            (node.computed_layout.offset.y - expected_y).abs() < 0.5,
            "cell {logical} y should be {expected_y}, got {}",
            node.computed_layout.offset.y
        );
    }
}

/// Scrolling shifts the mounted window forward but keeps the count bounded.
#[test]
fn lazy_grid_scroll_shifts_visible_window() {
    let (mut app, id) = setup_virtualized();
    let initial_built = with_lg(&app, id, |lg| lg.built_count());

    app.wheel(0.0, 1000.0, 200.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let after_built = with_lg(&app, id, |lg| lg.built_count());
    // First mounted index should be near row 10 - overscan 2 → row 8 → index 32.
    let first = with_lg(&app, id, |lg| lg.first_mounted_index().unwrap_or(0));
    assert!(
        (24..=40).contains(&first),
        "first mounted index after 1000px scroll should be ~32, got {first}"
    );
    assert!(
        (after_built as i64 - initial_built as i64).abs() <= 8,
        "mounted count should stay bounded after scroll (initial={initial_built}, after={after_built})"
    );
}

// ===========================================================================
// Regression: a reactive itemCount must survive a shrink→grow cycle (the
// LazyList twin bug — the declared count, not `visible.len()`, is the
// authoritative window bound).
// ===========================================================================

/// 100 → 8 → 100: after grow-back the viewport rows must re-mount and the
/// content extent must cover all 25 rows again.
#[test]
fn lazy_grid_reactive_item_count_grow_after_shrink_remounts_tail() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(
        r#"use tur::{ mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ Container, Expanded, LazyGrid };



fn cell(i: u64) -> opaque {
    let b = Container().width_height(100.0, 100.0).color(0xC8C8C8FFu64);
    return b.build();
}

entry fn start() -> u64 {
    let count = rs_source_f64();
    rs_set_f64(count, 100.0);
    let mut lg = LazyGrid().item_builder(cell).count(count).max_cross(100.0).aspect(1.0).query_key("lg").build();
    let lg = lg;
    let root = Expanded().flex(1.0).child(lg).build();
    mount(root);
    return count;
}

// The test drives count changes through the entry rail.
entry fn set_count(count: u64, n: f64) {
    rs_set_f64(count, n);
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let count_atom = app.rut_start_answer();
    let id = ElementNodeId::new(app.query_element(&["lg"]).unwrap().as_u64());

    // Shrink 100 → 8 (2 rows of 4 columns).
    app.call_rut_entry("set_count", count_atom, 8.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(id, |e| {
        let lg = e.cast::<LazyGridElement>().unwrap();
        assert!(
            lg.built_count() <= 8,
            "after shrink to 8, at most 8 cells should stay mounted, got {}",
            lg.built_count()
        );
    })
    .unwrap();

    // Grow back 8 → 100 (25 rows): the viewport window (6 visible rows × 4
    // columns + overscan) must re-mount, and the content extent must cover
    // all 25 rows again.
    app.call_rut_entry("set_count", count_atom, 100.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(id, |e| {
        let lg = e.cast::<LazyGridElement>().unwrap();
        let built = lg.built_count();
        assert!(
            built >= 24,
            "after grow-back to 100, the viewport rows must re-mount \
             (≥ 24 cells), got {built} — tail cells past the shrunken \
             count are frozen out"
        );
        let max = lg.max_scroll_extent();
        let expected = 25.0 * 100.0 - 600.0;
        assert!(
            (max - expected).abs() < 1.0,
            "content extent must cover all 25 rows again \
             (maxScrollExtent ≈ {expected}), got {max}"
        );
    })
    .unwrap();
}

/// Children stay ordered by logical index after scroll-up (move_child_before
/// preserves tree order for newly-mounted lower-index cells).
#[test]
fn lazy_grid_children_ordered_after_scroll() {
    let (mut app, id) = setup_virtualized();

    // Scroll down then back up.
    app.wheel(0.0, 2000.0, 200.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.wheel(0.0, -1000.0, 200.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let child_ids: Vec<_> = {
        let tree = app.element_tree();
        tree.get_element(id).unwrap().children.to_vec()
    };
    let mut prev: i64 = -1;
    for child_id in child_ids {
        let logical = with_lg(&app, id, move |lg| lg.visible_index_of(child_id))
            .expect("child should have a logical index");
        assert!(
            (logical as i64) > prev,
            "children vector out of order: prev={prev}, current={logical}"
        );
        prev = logical as i64;
    }
}

/// Scrolling past the end clamps at max_scroll_extent and never produces a
/// cell absurdly far off-screen.
#[test]
fn lazy_grid_scroll_clamps_at_content_end() {
    let (mut app, id) = setup_virtualized();

    app.wheel(0.0, 999_999.0, 200.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let max_extent = (10000.0_f64 / 4.0) * 100.0 - 600.0; // 2500 rows * 100 - 600
    let scroll = with_lg(&app, id, |lg| lg.scroll_offset());
    assert!(
        scroll <= max_extent + 1.0,
        "scroll should clamp at max extent ({max_extent}), got {scroll}"
    );
    assert!(scroll > 0.0);
}

/// Horizontal axis: cross axis = height → 6 rows of cells, scroll along x.
#[test]
fn lazy_grid_horizontal_axis() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(
        r#"
use tur::{ AXIS_HORIZONTAL, mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ Container, Expanded, LazyGrid };

fn cell(i: u64) -> opaque {
    let b = Container().width_height(100.0, 100.0).color(0xB4B4DCFFu64);
    return b.build();
}

entry fn start() -> u64 {
    let count = rs_source_f64();
    rs_set_f64(count, 1000.0);
    let mut lg = LazyGrid().item_builder(cell).count(count).axis(AXIS_HORIZONTAL).max_cross(100.0).aspect(1.0).query_key("lg").build();
    let lg = lg;
    let root = Expanded().flex(1.0).child(lg).build();
    mount(root);
    return count;
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = app.query_element(&["lg"]).expect("queryKey 'lg' not found");
    let id = ElementNodeId::new(id.as_u64());

    // viewport height 600, maxExtent 100 → 6 cross-axis slots.
    let cols = with_lg(&app, id, |lg| lg.cross_axis_count());
    assert_eq!(
        cols, 6,
        "horizontal grid should derive 6 cross-axis slots from height 600 / maxExtent 100"
    );

    // Scroll horizontally; verify a cell lands at viewport x≈0.
    app.wheel(500.0, 0.0, 200.0, 300.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    let found = {
        let tree = app.element_tree();
        let lg = tree.get_element(id).unwrap();
        lg.children.iter().any(|child| {
            let x = tree
                .get_element(ElementNodeId::new(child.as_u64()))
                .unwrap()
                .computed_layout
                .offset
                .x;
            (-100.0..=0.0).contains(&x)
        })
    };
    assert!(
        found,
        "expected at least one cell within one stride of viewport left edge after horizontal scroll"
    );
}

/// Parent's children count must equal the grid's mounted count (no
/// double-add / stale bookkeeping across scroll cycles).
#[test]
fn lazy_grid_parent_children_count_matches_mounted() {
    let (mut app, id) = setup_virtualized();
    for _ in 0..3 {
        app.wheel(0.0, 1500.0, 200.0, 300.0);
        app.wait_for_timeout(std::time::Duration::ZERO);
        app.wheel(0.0, -800.0, 200.0, 300.0);
        app.wait_for_timeout(std::time::Duration::ZERO);

        let parent_count = {
            let tree = app.element_tree();
            tree.get_element(id).unwrap().children.len()
        };
        let built = with_lg(&app, id, |lg| lg.built_count());
        assert_eq!(
            parent_count, built,
            "parent.children.len() ({parent_count}) should equal mounted count ({built})"
        );
    }
}

// ===========================================================================
// Column-count ceil semantics (Flutter parity): maxCrossAxisExtent is an
// inclusive UPPER bound on the cell cross size, so count = ceil(cross /
// maxExtent) — floor minted 2 columns of 217.5px in a 435px viewport where
// Flutter/boa produce 3 columns of 145px.
// ===========================================================================

/// 435px wide viewport, maxCrossAxisExtent 150 → ceil(435/150) = 3 columns
/// of 145px each.
#[test]
fn lazy_grid_column_count_ceils_max_extent_division() {
    let mut app = TurTestApp::new(435.0, 600.0).unwrap();
    app.load_rut_module(
        r#"
use tur::{ mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ Container, Expanded, LazyGrid };

fn cell(i: u64) -> opaque {
    let b = Container().width_height(145.0, 145.0).color(0xC8C8C8FFu64);
    return b.build();
}

entry fn start() -> u64 {
    let count = rs_source_f64();
    rs_set_f64(count, 12.0);
    let mut lg = LazyGrid().item_builder(cell).count(count).max_cross(150.0).aspect(1.0).query_key("lg").build();
    let lg = lg;
    let root = Expanded().flex(1.0).child(lg).build();
    mount(root);
    return count;
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = ElementNodeId::new(app.query_element(&["lg"]).unwrap().as_u64());

    let cols = with_lg(&app, id, |lg| lg.cross_axis_count());
    assert_eq!(cols, 3, "435/150 must ceil to 3 columns (floor gives 2)");

    // First mounted row (children are logical-index ordered): cells 145 wide
    // at x = 0, 145, 290.
    let tree = app.element_tree();
    let lg = tree.get_element(id).unwrap();
    let row: Vec<(f64, f64)> = lg.children[..3]
        .iter()
        .map(|&c| {
            let n = tree.get_element(ElementNodeId::new(c.as_u64())).unwrap();
            (n.computed_layout.size.width, n.computed_layout.offset.x)
        })
        .collect();
    assert_eq!(
        row,
        vec![(145.0, 0.0), (145.0, 145.0), (145.0, 290.0)],
        "first row must be 3 cells of 145px at column offsets"
    );
}

/// Exact-multiple boundary: 435/145 = 3.0 exactly must stay 3 columns (a
/// float fuzz above the integer would ceil to a phantom 4th), across an
/// item-count change — `total_lines = item_count.div_ceil(count)` and the
/// remount path must key off the same stable count.
#[test]
fn lazy_grid_exact_multiple_stays_at_exact_count() {
    let mut app = TurTestApp::new(435.0, 600.0).unwrap();
    app.load_rut_module(
        r#"
use tur::{ mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ Container, Expanded, LazyGrid };

fn cell(i: u64) -> opaque {
    let b = Container().width_height(145.0, 145.0).color(0xB4B4DCFFu64);
    return b.build();
}

entry fn start() -> u64 {
    let count = rs_source_f64();
    rs_set_f64(count, 6.0);
    let mut lg = LazyGrid().item_builder(cell).count(count).max_cross(145.0).aspect(1.0).query_key("lg").build();
    let lg = lg;
    let root = Expanded().flex(1.0).child(lg).build();
    mount(root);
    return count;
}

// The test drives count changes through the entry rail.
entry fn set_count(count: u64, n: f64) {
    rs_set_f64(count, n);
}
"#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let count_atom = app.rut_start_answer();
    let id = ElementNodeId::new(app.query_element(&["lg"]).unwrap().as_u64());

    // 6 items / 3 columns = exactly 2 rows; maxScrollExtent clamps at 0
    // (2*145 = 290 < 600 viewport).
    let cols = with_lg(&app, id, |lg| lg.cross_axis_count());
    assert_eq!(cols, 3, "435/145 = 3.0 exactly must stay 3 columns");

    // 30 items / 3 columns = exactly 10 rows → 10*145 - 600 = 850.
    app.call_rut_entry("set_count", count_atom, 30.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.with_element(id, |e| {
        let lg = e.cast::<LazyGridElement>().unwrap();
        assert_eq!(
            lg.cross_axis_count(),
            3,
            "count change must not re-derive the column count (still exactly 3)"
        );
        let max = lg.max_scroll_extent();
        let expected = 10.0 * 145.0 - 600.0;
        assert!(
            (max - expected).abs() < 1.0,
            "content extent must cover exactly 10 rows of 145px \
             (maxScrollExtent ≈ {expected}), got {max}"
        );
    })
    .unwrap();
}
