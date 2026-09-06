use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::{JsArray, JsFunction};
use boa_engine::{JsString, JsValue};

use crate::core::edgy::reactive::AnyReadable;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::render::brush::Brush;
use crate::core::view::{Lifecycle, Val, View, ViewCx, extract_view, read_atom_raw};

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
// TableView — the user's declaration. Pure Rust after parsing.
//
// `columns` is static sizing data. `rows` is the reactive data atom.
// `build` is a JS function `(item, index) => Element[]` invoked once per
// row to produce that row's cells; `buildHeader` is a JS function
// `() => Element[]` invoked ONCE at build time for the header cells.
// The sizing/chrome props are optional reactives.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TableView {
    pub(crate) columns: Vec<TableColumnDef>,
    pub(crate) rows: AnyReadable,
    pub(crate) build: JsFunction,
    pub(crate) build_header: Option<JsFunction>,
    pub(crate) header_extent: Option<Val<f64>>,
    pub(crate) row_extent: Option<Val<f64>>,
    pub(crate) row_spacing: Option<Val<f64>>,
    pub(crate) stripe_color: Option<Val<Brush>>,
    pub(crate) divider_color: Option<Val<Brush>>,
    pub(crate) divider_thickness: Option<Val<f64>>,
    pub(crate) query_key: Option<Vec<String>>,
}

/// Extract the cell specs from a JS value that the `build` / `buildHeader`
/// fns returned (an array of view handles). Positional: index `i` maps to
/// column `i`. `null` / `undefined` entries are placeholders (an empty cell
/// box — the column advances but nothing is mounted); malformed entries are
/// treated the same way.
pub(super) fn specs_from_array(v: &JsValue, boa: &mut Context) -> Vec<Option<Rc<dyn View>>> {
    let Some(arr) = v
        .as_object()
        .and_then(|o| JsArray::from_object(o.clone()).ok())
    else {
        return Vec::new();
    };
    let len = arr.length(boa).unwrap_or(0);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len as i64 {
        let spec = match arr.at(i, boa) {
            Ok(el) if !el.is_null_or_undefined() => extract_view(&el),
            _ => None,
        };
        out.push(spec);
    }
    out
}

/// Length of a JS value if it is an array (else 0).
pub(super) fn array_len(v: &JsValue, boa: &mut Context) -> usize {
    v.as_object()
        .and_then(|o| JsArray::from_object(o.clone()).ok())
        .and_then(|a| a.length(boa).ok())
        .unwrap_or(0) as usize
}

/// Invoke the row `build(item, index)` closure and materialize its cell
/// specs under `parent`. Returns `(column, node)` pairs — null placeholders
/// skip their column, and entries beyond the declared column count are
/// ignored. A throwing builder yields no cells for that row — layout
/// continues (same tolerance as `LazyList`/`Each`).
pub(super) fn build_row_cells(
    view: &TableView,
    item: &JsValue,
    index: u64,
    boa: &mut Context,
    cx: &mut dyn ViewCx,
    parent: NodeId,
) -> Vec<(usize, NodeId)> {
    let Ok(result) = view.build.call(
        &JsValue::undefined(),
        &[item.clone(), JsValue::from(index as f64)],
        boa,
    ) else {
        return Vec::new();
    };
    specs_from_array(&result, boa)
        .into_iter()
        .zip(0..)
        .filter_map(|(spec, col)| {
            let spec = spec?;
            (col < view.columns.len()).then_some((col, spec.build(cx, boa, parent)))
        })
        .collect()
}

/// Read the current `rows` array from the store and materialize every row's
/// cells under `parent`. Returns the per-row cell ids in array order.
pub(super) fn build_all_rows(
    view: &TableView,
    raw: &JsValue,
    boa: &mut Context,
    cx: &mut dyn ViewCx,
    parent: NodeId,
) -> Vec<Vec<(usize, NodeId)>> {
    let Some(arr) = raw
        .as_object()
        .and_then(|o| JsArray::from_object(o.clone()).ok())
    else {
        return Vec::new();
    };
    let len = arr.length(boa).unwrap_or(0);

    let mut rows = Vec::with_capacity(len as usize);
    for i in 0..len as i64 {
        let Ok(item) = arr.at(i, boa) else {
            rows.push(Vec::new());
            continue;
        };
        rows.push(build_row_cells(view, &item, i as u64, boa, cx, parent));
    }
    rows
}

impl View for TableView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());

        // Header cells first — they precede the body cells in child order.
        // `buildHeader` runs exactly once, here; reactive header *content*
        // flows through `Val` props inside the returned cells. Built before
        // the element node exists, so their self-links to `id` no-op — they
        // are linked explicitly after `insert_node` (LazyList's order).
        let header_cells: Vec<(usize, NodeId)> = self
            .build_header
            .as_ref()
            .and_then(|f| f.call(&JsValue::undefined(), &[], boa).ok())
            .map(|result| specs_from_array(&result, boa))
            .map(|specs| {
                specs
                    .into_iter()
                    .zip(0..)
                    .filter_map(|(spec, col)| {
                        let spec = spec?;
                        (col < self.columns.len()).then_some((col, spec.build(cx, boa, id.into())))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Body rows — built eagerly from the current array value.
        let raw = read_atom_raw(cx, self.rows, boa);
        let rows_len = array_len(&raw, boa);
        let row_cells = build_all_rows(self, &raw, boa, cx, id.into());

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
            boa,
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
    /// Header cells as `(column, node)` pairs (empty when `buildHeader` is
    /// absent). The column index skips null placeholders.
    pub(crate) header_cells: Vec<(usize, NodeId)>,
    /// Per-row cells as `(column, node)` pairs, in rows-array order.
    pub(crate) row_cells: Vec<Vec<(usize, NodeId)>>,
    /// Identity of the rows-array value the current `row_cells` were built
    /// from (`JsValue` equality — reference identity for arrays). Drives the
    /// rebuild decision during layout. Note: mutating the same array object
    /// in place (same identity, same length) is not observed — write a fresh
    /// array, the `Each`-idiomatic `store.set(rows$, [...items, x])`.
    pub(crate) rows_stamp: Option<JsValue>,
    /// Length of the built rows array (guards same-object length changes).
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
// Factory — parse props into a spec.
// ---------------------------------------------------------------------------

/// Parse `columns` — a non-empty JS array of `{ width?, flex?, minWidth? }`
/// entries. Returns `None` when missing / not an array / empty.
fn prop_columns(props: &JsObject, ctx: &mut Context) -> Option<Vec<TableColumnDef>> {
    let v = props.get(JsString::from("columns"), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()?;
    if len == 0 {
        return None;
    }

    fn prop_num(entry: &JsObject, name: &str, ctx: &mut Context) -> Option<f64> {
        entry
            .get(JsString::from(name), ctx)
            .ok()?
            .as_number()
            .filter(|n| n.is_finite())
    }

    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len as i64 {
        let Ok(entry) = arr.at(i, ctx) else {
            continue;
        };
        let Some(entry_obj) = entry.as_object() else {
            continue;
        };
        out.push(TableColumnDef {
            width: prop_num(&entry_obj, "width", ctx).map(|w| w.max(0.0)),
            flex: prop_num(&entry_obj, "flex", ctx).map(|f| f.max(0.0)),
            min_width: prop_num(&entry_obj, "minWidth", ctx).map(|m| m.max(0.0)),
        });
    }
    if out.is_empty() { None } else { Some(out) }
}

impl TableView {
    /// Build a `TableView` from a JS props object. Returns `None` when a
    /// required prop is missing or malformed: a non-empty `columns` array,
    /// the `rows` readable, and the `build` function.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let columns = prop_columns(props, ctx)?;
        let mut p = JsProps::new(props, ctx);
        Some(TableView {
            columns,
            rows: p.readable("rows")?,
            build: p.function("build")?,
            build_header: p.function("buildHeader"),
            header_extent: p.val::<f64>("headerExtent"),
            row_extent: p.val::<f64>("rowExtent"),
            row_spacing: p.val::<f64>("rowSpacing"),
            stripe_color: p.val::<Brush>("stripeColor"),
            divider_color: p.val::<Brush>("dividerColor"),
            divider_thickness: p.val::<f64>("dividerThickness"),
            query_key: p.query_key("queryKey"),
        })
    }
}
