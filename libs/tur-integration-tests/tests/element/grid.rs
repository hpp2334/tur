use tur_engine::builtin_plugins::layout::GridElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// Helper: load a Grid inline with the given props + child count, render, and
/// return the Grid element's id. The fixture authors `count` gray tiles
/// through the `el_grid` builder rows.
fn setup_grid(
    width: f64,
    height: f64,
    grid_opts: &GridOpts,
    count: usize,
) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(width, height).unwrap();
    app.load_rut_module(&format!(
        r#"

use tur_kit::handles::{{ mount }};
use tur_kit::layout::box::{{ Container }};
use tur_kit::layout::grid_table::{{ Grid }};

fn tile() -> View {{
    let b = Container().width_height(10.0, 10.0).color(0xC8C8C8FFu64);
    return b.build();
}}

entry fn start() {{
    let mut g = Grid().query_key("g").max_cross({max_cross});
{aspect}{extent}{spacing}    let mut i = 0;
    while (i < {count}) {{
        g.child(tile());
        i += 1;
    }}
    mount(g.build());
}}
"#,
        max_cross = grid_opts.max_cross,
        aspect = grid_opts
            .aspect
            .map(|a| format!(
                "    g.aspect({a});
"
            ))
            .unwrap_or_default(),
        extent = grid_opts
            .main_extent
            .map(|e| format!(
                "    g.main_extent({e});
"
            ))
            .unwrap_or_default(),
        spacing = grid_opts
            .spacing
            .map(|(c, m)| format!(
                "    g.spacing({c}, {m});
"
            ))
            .unwrap_or_default(),
        count = count,
    ))
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = app.query_element(&["g"]).expect("queryKey 'g' not found");
    (app, ElementNodeId::new(id.as_u64()))
}

/// The `grid_opts` crossing: max_cross is required; aspect / (cross, main)
/// spacing are optional.
#[derive(Clone, Copy)]
struct GridOpts {
    max_cross: f64,
    aspect: Option<f64>,
    main_extent: Option<f64>,
    spacing: Option<(f64, f64)>,
}

impl GridOpts {
    fn new(max_cross: f64) -> Self {
        GridOpts {
            max_cross,
            aspect: None,
            main_extent: None,
            spacing: None,
        }
    }
    fn aspect(mut self, a: f64) -> Self {
        self.aspect = Some(a);
        self
    }
    fn main_extent(mut self, e: f64) -> Self {
        self.main_extent = Some(e);
        self
    }
    fn spacing(mut self, cross: f64, main: f64) -> Self {
        self.spacing = Some((cross, main));
        self
    }
}

#[test]
fn grid_mounts_as_tur_grid() {
    let (app, id) = setup_grid(400.0, 600.0, &GridOpts::new(100.0), 4);
    let _ = id;
    let tree = app.element_tree();
    let root = tree.root_element().unwrap();
    assert_eq!(root.kind().unwrap(), ElementKind::new("tur_root"));
    let g = tree
        .get_element(ElementNodeId::new(root.children[0].as_u64()))
        .unwrap();
    assert_eq!(g.kind().unwrap(), ElementKind::new("tur_grid"));
}

/// 400px wide, maxExtent 100, no spacing → 4 columns of 100px each.
#[test]
fn grid_column_count_derived_from_max_extent() {
    let (app, id) = setup_grid(400.0, 600.0, &GridOpts::new(100.0), 8);

    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    // 8 children, 4 per row → 2 rows.
    assert_eq!(g.children.len(), 8);

    // Child 0: (0, 0), size 100x100.
    let c0 = tree
        .get_element(ElementNodeId::new(g.children[0].as_u64()))
        .unwrap();
    assert_eq!(c0.computed_layout.size.width, 100.0);
    assert_eq!(c0.computed_layout.size.height, 100.0);
    assert_eq!(c0.computed_layout.offset.x, 0.0);
    assert_eq!(c0.computed_layout.offset.y, 0.0);

    // Child 3 (last in row 0): x = 300.
    let c3 = tree
        .get_element(ElementNodeId::new(g.children[3].as_u64()))
        .unwrap();
    assert_eq!(c3.computed_layout.offset.x, 300.0);
    assert_eq!(c3.computed_layout.offset.y, 0.0);

    // Child 4 (first in row 1): (0, 100).
    let c4 = tree
        .get_element(ElementNodeId::new(g.children[4].as_u64()))
        .unwrap();
    assert_eq!(c4.computed_layout.offset.x, 0.0);
    assert_eq!(c4.computed_layout.offset.y, 100.0);
}

/// `childAspectRatio: 2` → cell_main = cell_cross / 2 = 50.
#[test]
fn grid_child_aspect_ratio_scales_main_axis() {
    let (app, id) = setup_grid(400.0, 600.0, &GridOpts::new(100.0).aspect(2.0), 4);
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    let c0 = tree
        .get_element(ElementNodeId::new(g.children[0].as_u64()))
        .unwrap();
    assert_eq!(c0.computed_layout.size.width, 100.0);
    assert_eq!(
        c0.computed_layout.size.height, 50.0,
        "cell height should be cell_cross / childAspectRatio = 100/2 = 50"
    );
}

/// `mainAxisExtent` overrides aspect-derived sizing.
#[test]
fn grid_main_axis_extent_overrides_aspect() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &GridOpts::new(100.0).aspect(2.0).main_extent(80.0),
        4,
    );
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    let c0 = tree
        .get_element(ElementNodeId::new(g.children[0].as_u64()))
        .unwrap();
    assert_eq!(c0.computed_layout.size.width, 100.0);
    assert_eq!(c0.computed_layout.size.height, 80.0);
}

/// Spacing shifts both the column pitch and the row pitch.
#[test]
fn grid_spacing_advances_positions() {
    // 400w, maxExtent 100 → 4 cols. crossAxisSpacing 10, mainAxisSpacing 10.
    // usable = 400 - 3*10 = 370. cell_cross = 370/4 = 92.5.
    // x positions: 0, 102.5, 205, 307.5. row pitch = 92.5 + 10 = 102.5.
    let (app, id) = setup_grid(400.0, 600.0, &GridOpts::new(100.0).spacing(10.0, 10.0), 8);
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();

    let c0 = tree
        .get_element(ElementNodeId::new(g.children[0].as_u64()))
        .unwrap();
    assert_eq!(c0.computed_layout.size.width, 92.5);
    assert_eq!(c0.computed_layout.size.height, 92.5);
    assert_eq!(c0.computed_layout.offset.x, 0.0);

    let c1 = tree
        .get_element(ElementNodeId::new(g.children[1].as_u64()))
        .unwrap();
    assert_eq!(c1.computed_layout.offset.x, 102.5);

    let c4 = tree
        .get_element(ElementNodeId::new(g.children[4].as_u64()))
        .unwrap();
    assert_eq!(c4.computed_layout.offset.x, 0.0);
    assert_eq!(c4.computed_layout.offset.y, 102.5);
}

/// Fewer children than columns → one row, no spillover.
#[test]
fn grid_fewer_children_than_columns() {
    let (app, id) = setup_grid(400.0, 600.0, &GridOpts::new(100.0), 2);
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    assert_eq!(g.children.len(), 2);
    let c1 = tree
        .get_element(ElementNodeId::new(g.children[1].as_u64()))
        .unwrap();
    assert_eq!(c1.computed_layout.offset.x, 100.0);
    assert_eq!(c1.computed_layout.offset.y, 0.0);
}

/// 435px wide, maxExtent 150 → ceil(435/150) = 3 columns of 145px each.
/// Flutter parity: `maxCrossAxisExtent` is an inclusive UPPER bound on the
/// cell cross size, so the division must round UP — floor yields 2 columns
/// of 217.5px and blows the bound (the operator audit's grid-case failure).
#[test]
fn grid_column_count_ceils_max_extent_division() {
    let (app, id) = setup_grid(435.0, 600.0, &GridOpts::new(150.0), 6);
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();

    // Row 0: children 0..3 at x = 0, 145, 290; each 145x145.
    let c0 = tree
        .get_element(ElementNodeId::new(g.children[0].as_u64()))
        .unwrap();
    assert_eq!(c0.computed_layout.size.width, 145.0);
    assert_eq!(c0.computed_layout.size.height, 145.0);
    assert_eq!(c0.computed_layout.offset.x, 0.0);

    let c1 = tree
        .get_element(ElementNodeId::new(g.children[1].as_u64()))
        .unwrap();
    assert_eq!(c1.computed_layout.offset.x, 145.0);

    // Child 3 wraps to row 1 (3 columns, not 2).
    let c3 = tree
        .get_element(ElementNodeId::new(g.children[3].as_u64()))
        .unwrap();
    assert_eq!(c3.computed_layout.offset.x, 0.0);
    assert_eq!(c3.computed_layout.offset.y, 145.0);

    app.with_element(id, |e| {
        let g = e.cast::<GridElement>().unwrap();
        assert_eq!(g.cross_axis_count(), 3);
    })
    .expect("element lookup");
}

/// Exact-multiple boundary: 435/145 = 3.0 exactly must resolve to 3 columns —
/// float fuzz above the integer would ceil to a phantom 4th column.
#[test]
fn grid_exact_multiple_stays_at_exact_count() {
    let (app, id) = setup_grid(435.0, 600.0, &GridOpts::new(145.0), 6);
    app.with_element(id, |e| {
        let g = e.cast::<GridElement>().unwrap();
        assert_eq!(g.cross_axis_count(), 3);
    })
    .expect("element lookup");
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    let c0 = tree
        .get_element(ElementNodeId::new(g.children[0].as_u64()))
        .unwrap();
    assert_eq!(c0.computed_layout.size.width, 145.0);
    let c3 = tree
        .get_element(ElementNodeId::new(g.children[3].as_u64()))
        .unwrap();
    assert_eq!(
        c3.computed_layout.offset.x, 0.0,
        "child 3 must wrap to row 1 (3 columns, not 4)"
    );
}

/// The Grid element records the computed metrics for dev-tool tracing.
#[test]
fn grid_element_records_metrics() {
    let (app, id) = setup_grid(400.0, 600.0, &GridOpts::new(100.0), 8);
    app.with_element(id, |e| {
        let g = e.cast::<GridElement>().unwrap();
        assert_eq!(g.cross_axis_count(), 4);
    })
    .expect("element lookup");
}
