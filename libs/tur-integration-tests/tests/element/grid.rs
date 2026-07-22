use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::builtin_plugins::layout::GridElement;
use tur_integration_tests::TurTestApp;

/// Helper: load a Grid inline with the given props + child count, render, and
/// return the Grid element's id.
fn setup_grid(
    width: f64,
    height: f64,
    source: &str,
) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(width, height).unwrap();
    app.eval_module_source(source).unwrap();
    app.render();
    let id = app.query_element(&["g"]).expect("queryKey 'g' not found");
    (app, ElementNodeId::new(id.as_u64()))
}

fn color_tile_source(count: usize, grid_opts: &str) -> String {
    format!(
        r#"
        import {{ render, Grid, Container, createColor }} from "tur:std";
        render(Grid({{
            queryKey: ["g"],
            {grid_opts}
            children: Array.from({{ length: {count} }}, () =>
                Container({{ color: createColor(200, 200, 200, 255) }}),
            ),
        }}));
        "#,
    )
}

#[test]
fn grid_mounts_as_tur_grid() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(4, "maxCrossAxisExtent: 100,"),
    );
    let _ = id;
    let tree = app.element_tree();
    let root = tree.root_element().unwrap();
    assert_eq!(
        root.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_root")
    );
    let g = tree
        .get_element(ElementNodeId::new(root.children[0].as_u64()))
        .unwrap();
    assert_eq!(
        g.element.as_ref().unwrap().kind(),
        ElementKind::new("tur_grid")
    );
}

/// 400px wide, maxExtent 100, no spacing → 4 columns of 100px each.
#[test]
fn grid_column_count_derived_from_max_extent() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(8, "maxCrossAxisExtent: 100,"),
    );

    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    // 8 children, 4 per row → 2 rows.
    assert_eq!(g.children.len(), 8);

    // Child 0: (0, 0), size 100x100.
    let c0 = tree.get_element(ElementNodeId::new(g.children[0].as_u64())).unwrap();
    assert_eq!(c0.computed_layout.size.width, 100.0);
    assert_eq!(c0.computed_layout.size.height, 100.0);
    assert_eq!(c0.computed_layout.offset.x, 0.0);
    assert_eq!(c0.computed_layout.offset.y, 0.0);

    // Child 3 (last in row 0): x = 300.
    let c3 = tree.get_element(ElementNodeId::new(g.children[3].as_u64())).unwrap();
    assert_eq!(c3.computed_layout.offset.x, 300.0);
    assert_eq!(c3.computed_layout.offset.y, 0.0);

    // Child 4 (first in row 1): (0, 100).
    let c4 = tree.get_element(ElementNodeId::new(g.children[4].as_u64())).unwrap();
    assert_eq!(c4.computed_layout.offset.x, 0.0);
    assert_eq!(c4.computed_layout.offset.y, 100.0);
}

/// `childAspectRatio: 2` → cell_main = cell_cross / 2 = 50.
#[test]
fn grid_child_aspect_ratio_scales_main_axis() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(4, "maxCrossAxisExtent: 100, childAspectRatio: 2,"),
    );
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    let c0 = tree.get_element(ElementNodeId::new(g.children[0].as_u64())).unwrap();
    assert_eq!(c0.computed_layout.size.width, 100.0);
    assert_eq!(c0.computed_layout.size.height, 50.0,
        "cell height should be cell_cross / childAspectRatio = 100/2 = 50");
}

/// `mainAxisExtent` overrides aspect-derived sizing.
#[test]
fn grid_main_axis_extent_overrides_aspect() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(
            4,
            "maxCrossAxisExtent: 100, childAspectRatio: 2, mainAxisExtent: 80,",
        ),
    );
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    let c0 = tree.get_element(ElementNodeId::new(g.children[0].as_u64())).unwrap();
    assert_eq!(c0.computed_layout.size.width, 100.0);
    assert_eq!(c0.computed_layout.size.height, 80.0);
}

/// Spacing shifts both the column pitch and the row pitch.
#[test]
fn grid_spacing_advances_positions() {
    // 400w, maxExtent 100 → 4 cols. crossAxisSpacing 10, mainAxisSpacing 10.
    // usable = 400 - 3*10 = 370. cell_cross = 370/4 = 92.5.
    // x positions: 0, 102.5, 205, 307.5. row pitch = 92.5 + 10 = 102.5.
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(
            8,
            "maxCrossAxisExtent: 100, crossAxisSpacing: 10, mainAxisSpacing: 10,",
        ),
    );
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();

    let c0 = tree.get_element(ElementNodeId::new(g.children[0].as_u64())).unwrap();
    assert_eq!(c0.computed_layout.size.width, 92.5);
    assert_eq!(c0.computed_layout.size.height, 92.5);
    assert_eq!(c0.computed_layout.offset.x, 0.0);

    let c1 = tree.get_element(ElementNodeId::new(g.children[1].as_u64())).unwrap();
    assert_eq!(c1.computed_layout.offset.x, 102.5);

    let c4 = tree.get_element(ElementNodeId::new(g.children[4].as_u64())).unwrap();
    assert_eq!(c4.computed_layout.offset.x, 0.0);
    assert_eq!(c4.computed_layout.offset.y, 102.5);
}

/// Fewer children than columns → one row, no spillover.
#[test]
fn grid_fewer_children_than_columns() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(2, "maxCrossAxisExtent: 100,"),
    );
    let tree = app.element_tree();
    let g = tree.get_element(id).unwrap();
    assert_eq!(g.children.len(), 2);
    let c1 = tree.get_element(ElementNodeId::new(g.children[1].as_u64())).unwrap();
    assert_eq!(c1.computed_layout.offset.x, 100.0);
    assert_eq!(c1.computed_layout.offset.y, 0.0);
}

/// The Grid element records the computed metrics for dev-tool tracing.
#[test]
fn grid_element_records_metrics() {
    let (app, id) = setup_grid(
        400.0,
        600.0,
        &color_tile_source(8, "maxCrossAxisExtent: 100,"),
    );
    app.with_element(id, |e| {
        let g = e.cast::<GridElement>().unwrap();
        assert_eq!(g.cross_axis_count(), 4);
    })
    .expect("element lookup");
}
