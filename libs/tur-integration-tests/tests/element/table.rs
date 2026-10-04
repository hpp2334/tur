use tur_engine::core::element::{ElementKind, ElementNodeId};
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
use tur::{ mount, rs_list_new, rs_list_push, rs_set_value, rs_source_value, stf_put, stf_take };
use tur_kit::{ Column, Container, Table, TableCols };


let ROWS: u64 = 9;

fn cell(key: str) -> opaque {
    let b = Container().width_height(10.0, 10.0).color(0xC8C8C8FFu64).query_key(key);
    return b.build();
}

entry fn header_row() -> opaque {
    let col = Column().child(cell("h0")).child(cell("h1")).child(cell("h2"));
    return col.build();
}

// The row builder receives the row INDEX directly (the RutEntryBuilder
// face calls it with the row's position).
entry fn row_cell(i: u64) -> opaque {
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

entry fn start() {
    let rows = rs_source_value(rows_of({ROWS_PLACEHOLDER}));
    stf_put(ROWS, rows as f64);

    let cols = TableCols().fixed(100.0).flex(1.0, 40.0).flex(3.0, 0.0);

    let mut t = Table().columns(cols).rows_atom(rows).row_builder("row_cell").header_builder("header_row").query_key("t").build();
    mount(t);
}

entry fn set_rows(_a: u64, n: f64) {
    let atom = stf_take(ROWS) as u64;
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
