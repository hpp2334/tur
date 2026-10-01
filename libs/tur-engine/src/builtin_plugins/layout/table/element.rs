use std::rc::Rc;

use crate::builtin_plugins::lazy_container::item_builder::RutEntryBuilder;
use crate::core::edgy::reactive::AnyReadable;
use crate::core::edgy::value::Value;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::render::brush::Brush;
use crate::core::rut_runtime::opaque_to_view;
use crate::core::view::{Lifecycle, Val, View, ViewCx};
use rut_vm::OpaqueRef;

// ---------------------------------------------------------------------------
// TableColumnDef — one entry of the `columns` prop. Pure sizing data:
// a fixed `width` (px) and/or a `flex` share of the leftover width.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TableColumnDef {
    /// Fixed width in px. When set, the column takes no flex share.
    pub width: Option<f64>,
    /// Share of the leftover width, proportional to the total flex weight.
    /// Columns without `width` default to flex 1 when this is absent.
    pub flex: Option<f64>,
    /// Lower bound applied to the distributed share (default 0).
    pub min_width: Option<f64>,
}

impl TableColumnDef {
    /// The effective flex weight: explicit `flex`, else 1 for width-less
    /// columns, else 0 (fixed columns take no flex share). Negative weights
    /// are treated as 0 by the distributor.
    pub(crate) fn flex_weight(&self) -> f64 {
        match (self.flex, self.width) {
            (Some(f), _) => f.max(0.0),
            (None, None) => 1.0,
            (None, Some(_)) => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// TableView — the user's declaration. Pure Rust.
//
// `columns` is static sizing data. `rows` is the reactive data atom. The
// builders are rut entry fns invoked through the guarded flush-time VM face
// (the lazy-container `ItemBuilder::Rut` mechanism): a rut entry returns
// exactly one value, so the row builder is called once per CELL —
// `entry fn(row: u64, col: u64) -> opaque` — and the host assembles each
// row's cells column-wise; the optional header builder is
// `entry fn(col: u64) -> opaque`, invoked ONCE per header column at build
// time. A failed call degrades to the empty-cell placeholder. The
// sizing/chrome props are optional reactives.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TableView {
    pub(crate) columns: Vec<TableColumnDef>,
    pub(crate) rows: AnyReadable,
    pub(crate) build: RutEntryBuilder,
    pub(crate) build_header: Option<RutEntryBuilder>,
    pub(crate) header_extent: Option<Val<f64>>,
    pub(crate) row_extent: Option<Val<f64>>,
    pub(crate) row_spacing: Option<Val<f64>>,
    pub(crate) stripe_color: Option<Val<Brush>>,
    pub(crate) divider_color: Option<Val<Brush>>,
    pub(crate) divider_thickness: Option<Val<f64>>,
    pub(crate) query_key: Option<Vec<String>>,
}

/// One row's resolved cell specs — `None` placeholders skip their column.
pub(super) type RowSpecs = Vec<Option<Rc<dyn View>>>;

/// Resolve one cell spec through a rut entry-builder (the guarded
/// flush-time VM face). Failures degrade to `None` — the empty-cell
/// placeholder, exactly like a throwing JS builder cell.
fn resolve_cell<A>(entry: &RutEntryBuilder, args: A) -> Option<Rc<dyn View>>
where
    A: rut_vm::interp::CallArgs,
{
    let handle: Result<OpaqueRef, _> = entry.face.call(&entry.handles, &entry.name, args);
    handle.ok().and_then(|h| opaque_to_view(&h))
}

/// Resolve one body row's cell specs — one guarded entry call per column
/// (positional: the `(row, col)` call maps to column `col`).
fn resolve_row_specs(view: &TableView, index: u64) -> RowSpecs {
    (0..view.columns.len() as u64)
        .map(|col| resolve_cell(&view.build, (index, col)))
        .collect()
}

/// Resolve the header's cell specs (`entry fn(col) -> opaque`, once per
/// column). Absent header → no specs.
fn resolve_header_specs(view: &TableView) -> RowSpecs {
    let Some(header) = view.build_header.as_ref() else {
        return Vec::new();
    };
    (0..view.columns.len() as u64)
        .map(|col| resolve_cell(header, (col,)))
        .collect()
}

/// Build resolved row specs into the tree under `parent` (realm-free).
/// Returns `(column, node)` pairs — `None` placeholders skip their column,
/// and entries beyond the declared column count are ignored.
fn build_row_cells(
    view: &TableView,
    specs: RowSpecs,
    cx: &mut dyn ViewCx,
    parent: NodeId,
) -> Vec<(usize, NodeId)> {
    specs
        .into_iter()
        .zip(0..)
        .filter_map(|(spec, col)| {
            let spec = spec?;
            (col < view.columns.len()).then_some((col, spec.build(cx, parent)))
        })
        .collect()
}

/// Read the current `rows` list and materialize every row's cells under
/// `parent`. Returns the per-row cell ids in array order. Each row's specs
/// resolve through the guarded rut entry face (realm-free — the same rail
/// the lazy containers build items with), then build into the tree.
pub(super) fn build_all_rows(
    view: &TableView,
    rows: &Value,
    cx: &mut dyn ViewCx,
    parent: NodeId,
) -> Vec<Vec<(usize, NodeId)>> {
    let Some(items) = rows.as_list() else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .map(|(index, _)| build_row_cells(view, resolve_row_specs(view, index as u64), cx, parent))
        .collect()
}

impl View for TableView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());

        // Current rows value (realm-free — the native KV serves without a
        // realm).
        let raw = cx.store_read_only().read(self.rows);
        let rows_len = raw.as_list().map_or(0, <[Value]>::len);

        // Resolve specs eagerly (guarded rut face calls), then build the
        // resolved views into the tree (realm-free). The header builder
        // runs exactly once, here; reactive header *content* flows through
        // `Val` props inside the returned cells.
        let header_specs = resolve_header_specs(self);
        let resolved_rows: Vec<RowSpecs> = (0..rows_len)
            .map(|i| resolve_row_specs(self, i as u64))
            .collect();

        let header_cells: Vec<(usize, NodeId)> = header_specs
            .into_iter()
            .zip(0..)
            .filter_map(|(spec, col)| {
                let spec = spec?;
                (col < self.columns.len()).then_some((col, spec.build(cx, id.into())))
            })
            .collect();
        let row_cells: Vec<Vec<(usize, NodeId)>> = resolved_rows
            .into_iter()
            .map(|specs| build_row_cells(self, specs, cx, id.into()))
            .collect();

        cx.insert_node(
            id,
            AnyElement::new(TableElement {
                view: self.clone(),
                node_id: id,
                header_cells: header_cells.clone(),
                row_cells: row_cells.clone(),
                rows_stamp: Some(raw),
                rows_len,
                col_widths: Vec::new(),
                row_heights: Vec::new(),
                row_tops: Vec::new(),
                header_height: 0.0,
                painting: TablePainting::default(),
            }),
        );

        // Link the built cells in declaration order (header, then rows
        // row-major) — preserving child order for paint/hit-test walks.
        for &(_, cell) in &header_cells {
            cx.link_child(id.into(), cell);
        }
        for row in &row_cells {
            for &(_, cell) in row {
                cx.link_child(id.into(), cell);
            }
        }

        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// TableElement — the built element. Holds its spec plus transient layout
// state: resolved column widths, per-row heights/tops, and the resolved
// paint chrome (fill props are resolved during layout — paint never touches
// the store, mirroring Container).
// ---------------------------------------------------------------------------

/// Resolved chrome values needed by paint, filled during layout.
#[derive(Default, Clone)]
pub(crate) struct TablePainting {
    pub(crate) stripe: Option<Brush>,
    pub(crate) divider: Option<Brush>,
    pub(crate) divider_thickness: f64,
    /// The painted chrome width (the table's own laid-out width).
    pub(crate) width: f64,
    /// The extents applied during layout (tight row/header heights). When
    /// set, paint clips each row's / the header's cells to its box — cell
    /// content that doesn't fit the fixed extent (e.g. wrapped text)
    /// overflows visually instead of bleeding into the next row.
    pub(crate) header_extent: Option<f64>,
    pub(crate) row_extent: Option<f64>,
}

pub struct TableElement {
    pub(crate) view: TableView,
    pub(crate) node_id: ElementNodeId,
    /// Header cells as `(column, node)` pairs (empty when the header
    /// builder is absent). The column index skips `None` placeholders.
    pub(crate) header_cells: Vec<(usize, NodeId)>,
    /// Per-row cells as `(column, node)` pairs, in rows-array order.
    pub(crate) row_cells: Vec<Vec<(usize, NodeId)>>,
    /// Identity of the rows-list value the current `row_cells` were built
    /// from (`Value` equality — list payloads compare by `Rc` reference
    /// identity, like a fresh JS array). Drives the rebuild decision during
    /// layout. Note: mutating the same list in place (same identity, same
    /// length) is not observed — write a fresh list, the `Each`-idiomatic
    /// `rs_set_value(rows$, …)`.
    pub(crate) rows_stamp: Option<Value>,
    /// Length of the built rows list (guards same-identity length changes).
    pub(crate) rows_len: usize,
    pub(crate) col_widths: Vec<f64>,
    pub(crate) row_heights: Vec<f64>,
    pub(crate) row_tops: Vec<f64>,
    pub(crate) header_height: f64,
    pub(crate) painting: TablePainting,
}

impl Lifecycle for TableElement {}

impl ElementSubscribe for TableElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        cx.subscribe_readable(self.view.rows);
        if let Some(v) = self.view.header_extent.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.row_extent.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.row_spacing.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.stripe_color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.divider_color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.divider_thickness.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for TableElement {
    fn trace_label(&self) -> String {
        format!(
            "cols={} rows={} header={}",
            self.view.columns.len(),
            self.row_cells.len(),
            usize::from(!self.header_cells.is_empty())
        )
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        vec![
            ("colCount", TraceValue::Num(self.view.columns.len() as f64)),
            ("rowCount", TraceValue::Num(self.row_cells.len() as f64)),
            ("headerHeight", TraceValue::Num(self.header_height)),
        ]
    }
}

// ---------------------------------------------------------------------------
// Constructor — the rut rail (`core::rut_runtime`).
// ---------------------------------------------------------------------------

impl TableView {
    /// Rut-rail constructor (`core::rut_runtime`): entry-builder row/header
    /// faces + column geometry + the reactive rows atom. The optional
    /// sizing/chrome props default (absent).
    pub(crate) fn new_rut(
        columns: Vec<TableColumnDef>,
        rows: AnyReadable,
        build: RutEntryBuilder,
        build_header: Option<RutEntryBuilder>,
    ) -> Self {
        TableView {
            columns,
            rows,
            build,
            build_header,
            header_extent: None,
            row_extent: None,
            row_spacing: None,
            stripe_color: None,
            divider_color: None,
            divider_thickness: None,
            query_key: Some(vec!["rut".to_string(), "table".to_string()]),
        }
    }
}
