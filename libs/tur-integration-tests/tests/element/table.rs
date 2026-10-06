use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::render::brush::{Brush, Color};
use tur_engine::core::render::{CanvasOp, RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

/// Helper: load a Table inline with the given body, render, and return the
/// Table element's id. The source is auto-wrapped into `start({ store })`,
/// so the body can use `store` directly.
fn setup_table(width: f64, height: f64, source: &str) -> (TurTestApp, ElementNodeId) {
    let mut app = TurTestApp::new(width, height).unwrap();
    app.load_rut_module(source).unwrap();
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
fn table_source(rows: usize, _table_opts: &str) -> String {
    r#"
use tur::{ ctx_bridge, mount, rs_list_new, rs_list_push, rs_set_value, rs_source_value };
use tur_kit::{ Column, Container, Table, TableCols };


fn cell(key: str) -> opaque {
    let b = Container().width_height(10.0, 10.0).color(0xC8C8C8FFu64).query_key(key);
    return b.build();
}

fn header_row(_col: u64) -> opaque {
    let col = Column().child(cell("h0")).child(cell("h1")).child(cell("h2"));
    return col.build();
}

// The row builder receives `(row, col)` (the RutEntryBuilder face calls
// it per column; the row spans all three columns, so `col` is ignored).
fn row_cell(i: u64, _col: u64) -> opaque {
    let col = Column().child(cell(f"c0-{i}")).child(cell(f"c1-{i}")).child(cell(f"c2-{i}"));
    return col.build();
}

fn rows_of(n: u64) -> opaque {
    let list = rs_list_new();
    let mut i = 0;
    while (i < n as i32) {
        rs_list_push(list, f"row {i}");
        i += 1;
    }
    return list;
}

entry fn start() -> u64 {
    let rows: Readable<opaque> = Readable<opaque>.of(ctx_bridge(), rs_source_value(rows_of({ROWS_PLACEHOLDER})));

    let cols = TableCols().fixed(100.0).flex(1.0, 40.0).flex(3.0, 0.0);

    let mut t = Table().columns(cols).rows_atom(rows).row_builder(row_cell).header_builder(header_row).query_key("t").build();
    mount(t);
    return rows.atom_id();
}

fn set_rows(atom: u64, n: f64) {
    rs_set_value(atom, rows_of(n as u64));
}
"#
        .replace("{ROWS_PLACEHOLDER}", &rows.to_string())
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

// ---------------------------------------------------------------------------
// Declarative stripes + column extents (the `table_stripe` /
// `table_col_extent` rows).
// ---------------------------------------------------------------------------

/// Renderer that stashes each frame's command batch so tests can inspect
/// the recorded paint ops (the paint_culling pattern).
struct RecordingRenderer {
    last: Rc<RefCell<Vec<RenderCommand>>>,
}

impl Renderer for RecordingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        *self.last.borrow_mut() = commands.to_vec();
    }
}

/// Every `FillGeometry` op recorded for `id` this frame, in paint order.
fn fills_for(
    cmds: &[RenderCommand],
    id: ElementNodeId,
) -> Vec<(
    tur_engine::core::layout::Offset,
    tur_engine::core::layout::Geometry,
    Brush,
)> {
    cmds.iter()
        .filter_map(|c| match c {
            RenderCommand::Paint {
                id: pid, ops, ..
            } if *pid == id => Some(ops),
            _ => None,
        })
        .flatten()
        .filter_map(|op| match op {
            CanvasOp::FillGeometry {
                offset,
                geometry,
                brush,
            } => Some((*offset, geometry.clone(), brush.clone())),
            _ => None,
        })
        .collect()
}

/// A stripe table: 4 body rows of 30px-tall colorless cells, no header,
/// declarative stripes — even rows red, odd rows blue (the cells paint
/// nothing, so the table's own fills are exactly the stripes).
const STRIPE_TABLE_RUT: &str = r#"
use tur::{ ctx_bridge, mount, rs_list_new, rs_list_push, rs_source_value };
use tur_kit::{ Readable, Container, Table, TableCols };


fn body_cell(_row: u64, _col: u64) -> opaque {
    return Container().width_height(0.0, 30.0).build();
}

fn rows_of(n: u64) -> opaque {
    let list = rs_list_new();
    let mut i: u64 = 0;
    while (i < n) {
        rs_list_push(list, f"r{i}");
        i = i + 1;
    }
    return list;
}

entry fn start() {
    let rows: Readable<opaque> = Readable<opaque>.of(ctx_bridge(), rs_source_value(rows_of(4)));
    let cols = TableCols().extent(300.0);
    let t = Table()
        .columns(cols)
        .rows_atom(rows)
        .row_builder(body_cell)
        .stripes(0xFF0000FFu64, 0x0000FFFFu64)
        .query_key("t")
        .build();
    mount(t);
}
"#;

#[test]
fn stripe_rows_paint_per_parity_from_the_element() {
    let last = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        400.0,
        300.0,
        Box::new(RecordingRenderer {
            last: last.clone(),
        }),
    )
    .expect("app");
    app.load_rut_module(STRIPE_TABLE_RUT).expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let tid = q(&app, "t");
    let fills = fills_for(&last.borrow(), tid);
    assert_eq!(
        fills.len(),
        4,
        "the table element paints one stripe fill per body row, got {fills:?}"
    );
    let even = Brush::SolidColor(Color::rgba(255, 0, 0, 255));
    let odd = Brush::SolidColor(Color::rgba(0, 0, 255, 255));
    for (i, (offset, geometry, brush)) in fills.iter().enumerate() {
        let expected = if i % 2 == 0 { &even } else { &odd };
        assert_eq!(
            brush, expected,
            "row {i} stripe must follow row parity (even=red, odd=blue)"
        );
        let tur_engine::core::layout::Geometry::Rect(rect) = geometry else {
            panic!("stripe fill must be a rect, got {geometry:?}")
        };
        assert_eq!(rect.width, 400.0, "stripe spans the table's laid width");
        assert_eq!(rect.height, 30.0, "stripe covers exactly its row");
        assert_eq!(offset.y, (i as f64) * 30.0, "stripe sits on its row's top");
    }
}

#[test]
fn column_extent_honored_in_layout() {
    let mut app = TurTestApp::new(400.0, 300.0).expect("app");
    // The table-basic geometry: a 150 extent column, flex 1, flex 2
    // (min 120) — the leftover 250 splits 1:2.
    app.load_rut_module(
        r#"
use tur::{ ctx_bridge, mount, rs_list_new, rs_list_push, rs_source_value };
use tur_kit::{ Readable, Container, Table, TableCols };


fn body_cell(_row: u64, _col: u64) -> opaque {
    return Container().width_height(0.0, 30.0).build();
}

entry fn start() {
    let rows = rs_list_new();
    rs_list_push(rows, "a");
    let rows_atom: Readable<opaque> = Readable<opaque>.of(ctx_bridge(), rs_source_value(rows));
    let cols = TableCols().extent(150.0).flex(1.0, 0.0).flex(2.0, 120.0);
    let t = Table()
        .columns(cols)
        .rows_atom(rows_atom)
        .row_builder(body_cell)
        .query_key("t")
        .build();
    mount(t);
}
"#,
    )
    .expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let tid = q(&app, "t");
    let tree = app.element_tree();
    let cells = tree.children_of_element(tid);
    assert!(
        cells.len() >= 3,
        "row 0 lays one cell per column, got {}",
        cells.len()
    );
    // Cells get tight column widths: extent column at 150, the flex pair
    // splitting the 250 leftover 1:2.
    let widths: Vec<f64> = cells[..3]
        .iter()
        .map(|id| {
            tree.get_element(*id)
                .expect("cell")
                .computed_layout
                .size
                .width
        })
        .collect();
    assert_eq!(
        widths[0], 150.0,
        "the extent column lays out at its extent"
    );
    assert!(
        (widths[1] - 250.0 / 3.0).abs() < 0.01,
        "flex 1 column takes its share, got {}",
        widths[1]
    );
    assert!(
        (widths[2] - 500.0 / 3.0).abs() < 0.01,
        "flex 2 column takes its share, got {}",
        widths[2]
    );
}
