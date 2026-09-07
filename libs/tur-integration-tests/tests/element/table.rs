use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// Helper: load a Table inline with the given body, render, and return the
/// Table element's id. The source is auto-wrapped into `start({ store })`,
/// so the body can use `store` directly.
fn setup_table(width: f64, height: f64, source: &str) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(width, height).unwrap();
    app.eval_module_source(source).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let id = app.query_element(&["t"]).expect("queryKey 't' not found");
    (app, ElementNodeId::new(id.as_u64()))
}

/// Query a single queryKey and convert to an ElementNodeId.
fn q(app: &TurTestApp, key: &str) -> ElementNodeId {
    ElementNodeId::new(
        app.query_element(&[key])
            .unwrap_or_else(|| panic!("queryKey '{key}' not found"))
            .as_u64(),
    )
}

/// 3 columns × N rows of empty Containers (content-sized cells) + a 3-cell
/// header. Cells carry per-column query keys so tests can address them.
fn table_source(rows: usize, table_opts: &str) -> String {
    format!(
        r#"import {{ mount, Table, Container, source }} from "tur:std";
        const rows$ = source(Array.from({{ length: {rows} }}, (_, i) => ({{ i }})));
        // Test seam: rewrite the rows atom through the injected store.
        globalThis.__set = (n) => store.set(rows$, Array.from({{ length: n }}, (_, i) => ({{ i }})));
        mount(Table({{ queryKey: ["t"], rows: rows$, {table_opts} }})
            .columns([{{ width: 100 }}, {{ flex: 1, minWidth: 40 }}, {{ flex: 3 }}])
            .headerBuilder(() => [
                Container({{ queryKey: ["h0"] }}).build(),
                Container({{ queryKey: ["h1"] }}).build(),
                Container({{ queryKey: ["h2"] }}).build(),
            ])
            .rowBuilder((item) => [
                Container({{ queryKey: ["c0-" + item.i] }}).build(),
                Container({{ queryKey: ["c1-" + item.i] }}).build(),
                Container({{ queryKey: ["c2-" + item.i] }}).build(),
            ])
            .build());
        "#,
    )
}

#[test]
fn table_mounts_as_tur_table() {
    let (app, id) = setup_table(400.0, 600.0, &table_source(2, ""));
    let _ = id;
    let tree = app.element_tree();
    let root = tree.root_element().unwrap();
    assert_eq!(root.kind().unwrap(), ElementKind::new("tur_root"));
    let t = tree
        .get_element(ElementNodeId::new(root.children[0].as_u64()))
        .unwrap();
    assert_eq!(t.kind().unwrap(), ElementKind::new("tur_table"));
}

/// 400px wide, columns [fixed 100, flex 1 (min 40), flex 3] → leftover 300
/// splits 75 / 225. Header cells sit at x = 0 / 100 / 175.
#[test]
fn table_fixed_and_flex_column_widths() {
    let (app, _id) = setup_table(400.0, 600.0, &table_source(2, ""));

    let expect = [(0.0, 100.0), (100.0, 75.0), (175.0, 225.0)];
    for (col, (x, w)) in expect.iter().enumerate() {
        let h = q(&app, &format!("h{col}"));
        let tree = app.element_tree();
        let node = tree.get_element(h).unwrap();
        assert_eq!(node.computed_layout.offset.x, *x, "header col {col} x");
        assert_eq!(
            node.computed_layout.size.width, *w,
            "header col {col} width"
        );
    }
}

/// The column geometry is shared: body cells in every row land on the same
/// x offsets / widths as the header cells.
#[test]
fn table_rows_share_column_geometry() {
    let (app, _id) = setup_table(400.0, 600.0, &table_source(3, ""));

    let expect = [(0.0, 100.0), (100.0, 75.0), (175.0, 225.0)];
    for row in 0..3 {
        for (col, (x, w)) in expect.iter().enumerate() {
            let c = q(&app, &format!("c{col}-{row}"));
            let tree = app.element_tree();
            let node = tree.get_element(c).unwrap();
            assert_eq!(node.computed_layout.offset.x, *x, "row {row} col {col} x");
            assert_eq!(node.computed_layout.size.width, *w, "row {row} col {col} w");
        }
    }
}

/// With explicit extents the header sits at the top (height 30) and every
/// body row is 40 tall; rows stack below the header.
#[test]
fn table_fixed_extents() {
    let (app, _id) = setup_table(
        400.0,
        600.0,
        &table_source(2, "headerExtent: 30, rowExtent: 40,"),
    );

    // Header cells: y = 0, height 30 (tight constraints — height-less
    // Containers fill the tight extent).
    for col in 0..3 {
        let h = q(&app, &format!("h{col}"));
        let tree = app.element_tree();
        let node = tree.get_element(h).unwrap();
        assert_eq!(node.computed_layout.offset.y, 0.0);
        assert_eq!(node.computed_layout.size.height, 30.0);
    }
    // Row 0 at y = 30, row 1 at y = 70.
    for (row, y) in [(0, 30.0), (1, 70.0)] {
        let c = q(&app, &format!("c0-{row}"));
        let tree = app.element_tree();
        let node = tree.get_element(c).unwrap();
        assert_eq!(node.computed_layout.offset.y, y, "row {row} y");
        assert_eq!(node.computed_layout.size.height, 40.0, "row {row} height");
    }
}

/// Without extents, each row's height is the max intrinsic cell height.
#[test]
fn table_intrinsic_row_height_is_max_cell() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"import { mount, Table, Container, source } from "tur:std";
        const rows$ = source([{ i: 0 }, { i: 1 }]);
        mount(Table({ columns: [{ flex: 1 }, { flex: 1 }], rows: rows$ })
    .queryKey(["t"])
    .rowSpacing(0)
    .headerBuilder(() => [Container()
    .height(30)
    .queryKey(["h0"])
    .build(), Container()
     .height(30)
     .build()])
    .rowBuilder((item) => [
                Container()
                    .height(20)
                    .queryKey(["c0-" + item.i])
                    .build(),
                Container()
                    .height(50)
                    .queryKey(["c1-" + item.i])
                    .build(),
            ])
    .build());
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Header intrinsic = 30. Row heights = max(20, 50) = 50 → row 1 at 30+50.
    let h0 = q(&app, "h0");
    let tree = app.element_tree();
    let node = tree.get_element(h0).unwrap();
    assert_eq!(node.computed_layout.size.height, 30.0);

    let c0 = q(&app, "c0-1");
    let tree = app.element_tree();
    let node = tree.get_element(c0).unwrap();
    assert_eq!(
        node.computed_layout.offset.y, 80.0,
        "row 1 top = header 30 + row0 50"
    );
    // The short cell keeps its own intrinsic height (loose constraints).
    assert_eq!(node.computed_layout.size.height, 20.0);
}

/// Writing a new array to the `rows` source rebuilds the mounted rows
/// (Each-style rebuild-all): added rows appear, removed rows disappear.
#[test]
fn table_reactive_rows_rebuild() {
    let (mut app, _id) = setup_table(400.0, 600.0, &table_source(2, ""));

    let t = q(&app, "t");
    let tree = app.element_tree();
    let node = tree.get_element(t).unwrap();
    // 3 header cells + 2 rows × 3 cells.
    assert_eq!(node.children.len(), 9);

    // Push a third row.
    app.eval_js("globalThis.__set(3)");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let t = q(&app, "t");
    let tree = app.element_tree();
    let node = tree.get_element(t).unwrap();
    assert_eq!(node.children.len(), 12, "3 rows × 3 cells + 3 header cells");

    // The new row's cells exist and share column geometry.
    let c = q(&app, "c2-2");
    let tree = app.element_tree();
    let node = tree.get_element(c).unwrap();
    assert_eq!(node.computed_layout.offset.x, 175.0);

    // Shrink back to 1 row.
    app.eval_js("globalThis.__set(1)");
    app.wait_for_timeout(std::time::Duration::ZERO);
    let t = q(&app, "t");
    let tree = app.element_tree();
    let node = tree.get_element(t).unwrap();
    assert_eq!(node.children.len(), 6, "1 row × 3 cells + 3 header cells");
    assert!(
        app.query_element(&["c0-1"]).is_none(),
        "removed row unmounted"
    );
}

/// A row builder returning fewer cells than columns leaves the missing
/// trailing boxes empty — the present columns are unaffected.
#[test]
fn table_missing_cells_leave_empty_box() {
    let (app, _id) = setup_table(
        400.0,
        600.0,
        r#"import { mount, Table, Container, source } from "tur:std";
        const rows$ = source([{ i: 0 }, { i: 1 }]);
        mount(Table({ columns: [{ width: 100 }, { flex: 1 }, { flex: 1 }], rows: rows$ })
    .queryKey(["t"])
    .headerBuilder(() => [
                Container()
                    .queryKey(["h0"])
                    .build(),
                Container()
                    .queryKey(["h1"])
                    .build(),
                Container()
                    .queryKey(["h2"])
                    .build(),
            ])
    .rowBuilder((item) => [
                Container()
                    .queryKey(["c0-" + item.i])
                    .build(),
                null,
                Container()
                    .queryKey(["c2-" + item.i])
                    .build(),
            ])
    .build());
        "#,
    );

    // Present cells keep their column geometry…
    for row in 0..2 {
        let c0 = q(&app, &format!("c0-{row}"));
        let tree = app.element_tree();
        let node = tree.get_element(c0).unwrap();
        assert_eq!(node.computed_layout.offset.x, 0.0);
        assert_eq!(node.computed_layout.size.width, 100.0);
        let c2 = q(&app, &format!("c2-{row}"));
        let tree = app.element_tree();
        let node = tree.get_element(c2).unwrap();
        // col2 x = 100 + 150 = 250 (leftover 300 split evenly).
        assert_eq!(node.computed_layout.offset.x, 250.0);
        assert_eq!(node.computed_layout.size.width, 150.0);
    }
    // …and rows have 2 cells, not 3.
    let t = q(&app, "t");
    let tree = app.element_tree();
    let node = tree.get_element(t).unwrap();
    assert_eq!(node.children.len(), 7, "3 header + 2 rows × 2 cells");
}

/// `minWidth` clamps the flex distribution even when it overflows the
/// available width.
#[test]
fn table_min_width_clamps_flex_share() {
    let (app, _id) = setup_table(
        400.0,
        600.0,
        r#"import { mount, Table, Container, source } from "tur:std";
        const rows$ = source([{}]);
        mount(Table({ columns: [{ width: 350 }, { flex: 1, minWidth: 80 }], rows: rows$ })
    .queryKey(["t"])
    .headerBuilder(() => [
                Container()
                    .queryKey(["h0"])
                    .build(),
                Container()
                    .queryKey(["h1"])
                    .build(),
            ])
    .rowBuilder(() => [Container()
    .queryKey(["c0-0"])
    .build(), Container()
     .queryKey(["c1-0"])
     .build()])
    .build());
        "#,
    );

    let h1 = q(&app, "h1");
    let tree = app.element_tree();
    let node = tree.get_element(h1).unwrap();
    // Leftover is 50 but minWidth forces 80.
    assert_eq!(node.computed_layout.size.width, 80.0);
    assert_eq!(node.computed_layout.offset.x, 350.0);
}
