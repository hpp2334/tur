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
use tur_kit::{ Column, Container, ListHandle, MutationCtx, Source, Table, TableCols, entry_ctx, list_new, mount, source };

fn cell(key: str) -> View {
    let b = Container().width_height(10.0, 10.0).color(0xC8C8C8FFu64).query_key(key);
    return b.build();
}

fn header_row(_col: u64) -> View {
    let col = Column().child(cell("h0")).child(cell("h1")).child(cell("h2"));
    return col.build();
}

// The row builder receives `(row, col)` (the RutEntryBuilder face calls
// it per column; the row spans all three columns, so `col` is ignored).
fn row_cell(i: u64, _col: u64) -> View {
    let col = Column().child(cell(f"c0-{i}")).child(cell(f"c1-{i}")).child(cell(f"c2-{i}"));
    return col.build();
}

fn rows_of(n: u64) -> ListHandle {
    let list: ListHandle = list_new();
    let mut i = 0;
    while (i < n as i32) {
        list.push(f"row {i}");
        i += 1;
    }
    return list;
}

entry fn start() -> u64 {
    let rows = source<opaque>(rows_of({ROWS_PLACEHOLDER}).raw());

    let cols = TableCols().fixed(100.0).flex(1.0, 40.0).flex(3.0, 0.0);

    let mut t = Table().columns(cols).rows_atom(rows).row_builder(row_cell).header_builder(header_row).query_key("t").build();
    mount(t);
    return rows.atom_id();
}

fn set_rows(atom: u64, n: f64) {
    entry_ctx().set<opaque>(atom, rows_of(n as u64).raw());
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
            RenderCommand::Paint { id: pid, ops, .. } if *pid == id => Some(ops),
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
use tur_kit::{ Container, ListHandle, Source, Table, TableCols, list_new, mount, source };

fn body_cell(_row: u64, _col: u64) -> View {
    return Container().width_height(0.0, 30.0).build();
}

fn rows_of(n: u64) -> ListHandle {
    let list: ListHandle = list_new();
    let mut i: u64 = 0;
    while (i < n) {
        list.push(f"r{i}");
        i = i + 1;
    }
    return list;
}

entry fn start() {
    let rows = source<opaque>(rows_of(4).raw());
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
        Box::new(RecordingRenderer { last: last.clone() }),
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
use tur_kit::{ Container, ListHandle, Source, Table, TableCols, list_new, mount, source };

fn body_cell(_row: u64, _col: u64) -> View {
    return Container().width_height(0.0, 30.0).build();
}

entry fn start() {
    let rows: ListHandle = list_new();
    rows.push("a");
    let rows_atom = source<opaque>(rows.raw());
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
    assert_eq!(widths[0], 150.0, "the extent column lays out at its extent");
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

// ===========================================================================
// The table-reactive corpus case: the 300ms fake load, the sortable
// PLANET/MOONS/GRAVITY headers (both directions), the active-state label
// markers, and the "Loaded N rows" status line. (boa's empty body is its
// own defect — Appendix A; the rows WORK here.)
// ===========================================================================

/// The concatenated span text of the first TextElement under `qk` (the
/// keyed cell wraps its Text — the countdown get_text pattern, extended
/// to walk a short subtree).
fn case_text(app: &TurTestApp, qk: &[&str]) -> String {
    use tur_engine::builtin_plugins::text::elements::TextElement;
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let id = ElementNodeId::new(id.as_u64());
    let tree = app.element_tree();
    let mut stack = vec![id];
    while let Some(id) = stack.pop() {
        let text = app
            .with_element(id, |e| {
                e.cast::<TextElement>()
                    .map(|t| {
                        t.spans()
                            .iter()
                            .map(|s| s.text.as_str())
                            .collect::<String>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if !text.is_empty() {
            return text;
        }
        let node = tree.get_element(id).unwrap();
        for c in &node.children {
            stack.push(ElementNodeId::new(c.as_u64()));
        }
    }
    panic!("no text under {qk:?}");
}

fn click_case(app: &mut TurTestApp, qk: &[&str]) {
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let id = ElementNodeId::new(id.as_u64());
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
}

fn build_table_reactive() -> TurTestApp {
    let mut app = TurTestApp::new(435.0, 600.0).unwrap();
    app.load_bundle("table-reactive").unwrap();
    app
}

#[test]
fn table_reactive_loads_rows_after_the_fake_delay() {
    let mut app = build_table_reactive();

    // Boot: the header + status render, the body is empty (boa's shape).
    assert_eq!(case_text(&app, &["tr", "status"]), "Loading…");
    assert!(app.query_element(&["tr", "row0", "c0"]).is_none());

    // The 300ms fake fetch elapses: 8 rows flow in, moons-ascending
    // (the boot sort state).
    app.wait_for_timeout(std::time::Duration::from_millis(350));
    assert_eq!(
        case_text(&app, &["tr", "status"]),
        "Loaded 8 rows · click a header to sort"
    );
    assert_eq!(case_text(&app, &["tr", "row0", "c0"]), "Mercury");
    assert_eq!(case_text(&app, &["tr", "row1", "c0"]), "Venus");
}

#[test]
fn table_reactive_headers_sort_both_ways() {
    let mut app = build_table_reactive();
    app.wait_for_timeout(std::time::Duration::from_millis(350));
    assert_eq!(case_text(&app, &["tr", "row0", "c0"]), "Mercury");

    // PLANET: ascending — the active header carries the ^ marker.
    click_case(&mut app, &["hdr", "name"]);
    assert_eq!(case_text(&app, &["tr", "row0", "c0"]), "Earth");
    assert_eq!(case_text(&app, &["hdr", "name"]), "PLANET ^");

    // PLANET again: the direction flips — v, Venus first.
    click_case(&mut app, &["hdr", "name"]);
    assert_eq!(case_text(&app, &["tr", "row0", "c0"]), "Venus");
    assert_eq!(case_text(&app, &["hdr", "name"]), "PLANET v");

    // GRAVITY: a new key resets to ascending (Mercury 3.7 wins the tie
    // over Mars — ties keep their order).
    click_case(&mut app, &["hdr", "gravity"]);
    assert_eq!(case_text(&app, &["tr", "row0", "c0"]), "Mercury");
    assert_eq!(case_text(&app, &["hdr", "gravity"]), "GRAVITY (m/s²) ^");
    assert_eq!(case_text(&app, &["hdr", "name"]), "PLANET");
}
