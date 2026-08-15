use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsValue};

use crate::core::edgy::mutation::IntoJsArgs;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{
    AnyElement, ElementOnWheel, ElementOnWheelContext, ElementTrace, TraceValue, WheelEvent,
};
use crate::core::js_runtime::JsProps;
use crate::core::layout::Axis;
use crate::core::view::{Val, View, ViewCx, extract_view, read_val};

use crate::builtin_plugins::scroll::ScrollPosition;

/// The default number of items built up-front when no viewport information is
/// available yet (i.e. during the initial `build()` before the first layout
/// pass). Once layout runs and we know the real viewport size, the range is
/// recomputed and unmounted items are pruned.
const INITIAL_BUILD_COUNT: u64 = 20;

// ---------------------------------------------------------------------------
// LazyGridView — the user's declaration.
//
// `axis`, `itemCount`, `overscan`, `maxCrossAxisExtent`, `childAspectRatio`,
// `mainAxisExtent`, and the spacing props are reactive (`Val<T>`). `builder`
// is a JS function `(index) => Element`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LazyGridView {
    pub(crate) axis: Option<Val<Axis>>,
    pub(crate) item_count: Val<u64>,
    pub(crate) overscan: Option<Val<u64>>,
    pub(crate) max_cross_axis_extent: Val<f64>,
    pub(crate) child_aspect_ratio: Option<Val<f64>>,
    pub(crate) main_axis_extent: Option<Val<f64>>,
    pub(crate) cross_axis_spacing: Option<Val<f64>>,
    pub(crate) main_axis_spacing: Option<Val<f64>>,
    pub(crate) builder: JsFunction,
    pub(crate) query_key: Option<Vec<String>>,
}

impl View for LazyGridView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = cx.alloc_node().as_element_id();

        let axis = self
            .axis
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or(Axis::Vertical);
        let item_count = read_val(cx, &self.item_count, boa).unwrap_or(0);
        let overscan = self
            .overscan
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or(3);

        // Build only the first INITIAL_BUILD_COUNT items (or fewer if
        // item_count is smaller). After the first layout, the remount pass
        // adjusts the mounted set to match the actual viewport.
        let initial_count = item_count.min(INITIAL_BUILD_COUNT);
        let builder = self.builder.clone();
        let mut visible: Vec<(u64, NodeId)> = Vec::new();
        for index in 0..initial_count {
            let Some(spec) = build_item_spec(&builder, index, boa) else {
                continue;
            };
            let item_id = spec.build(cx, boa, id.into());
            visible.push((index, item_id));
        }
        let item_ids: Vec<NodeId> = visible.iter().map(|&(_, id)| id).collect();

        cx.insert_node(
            id,
            AnyElement::with_wheel(LazyGridElement {
                view: self.clone(),
                node_id: id,
                axis,
                overscan,
                position: ScrollPosition::new(),
                cross_axis_count: 1,
                cell_cross: 0.0,
                cell_main: 0.0,
                stride_main: 0.0,
                cross_axis_spacing: 0.0,
                main_axis_spacing: 0.0,
                visible,
                reported_start: 0,
                reported_end: 0,
            })
            .with_callbacks(),
            boa,
        );

        for item_id in item_ids {
            cx.link_child(id.into(), item_id);
        }
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

/// Invoke the JS builder closure for `index`, returning the produced spec.
fn build_item_spec(builder: &JsFunction, index: u64, boa: &mut Context) -> Option<Rc<dyn View>> {
    let result = builder
        .call(&JsValue::undefined(), &[JsValue::from(index as f64)], boa)
        .ok()?;
    extract_view(&result)
}

// ---------------------------------------------------------------------------
// LazyGridElement — the built element.
//
// Because cells are uniform size, positioning is analytic: each cell's offset
// is derived directly from its logical index (`line = index / count`,
// `slot = index % count`), with no persistent positioning anchor needed
// (contrast LazyList's `first_mounted_offset` for variable-height items).
// ---------------------------------------------------------------------------

pub struct LazyGridElement {
    pub(crate) view: LazyGridView,
    pub(crate) node_id: ElementNodeId,
    pub(crate) axis: Axis,
    pub(crate) overscan: u64,
    pub(crate) position: ScrollPosition,
    /// Cells per cross-axis line (column count for a vertical grid). Cached
    /// for change detection: when the viewport-derived count changes (resize
    /// crossing a max-extent boundary), the index→line/slot mapping shifts and
    /// the whole visible set must be remounted from scratch.
    pub(crate) cross_axis_count: usize,
    pub(crate) cell_cross: f64,
    pub(crate) cell_main: f64,
    /// Row pitch: `cell_main + main_axis_spacing`.
    pub(crate) stride_main: f64,
    pub(crate) cross_axis_spacing: f64,
    pub(crate) main_axis_spacing: f64,
    /// `(index, built node id)` for every cell currently in the tree. Kept
    /// sorted by index.
    pub(crate) visible: Vec<(u64, NodeId)>,
    pub(crate) reported_start: u64,
    pub(crate) reported_end: u64,
}

impl LazyGridElement {
    pub fn scroll_offset(&self) -> f64 {
        self.position.pixels()
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// The declared item count (static value only; reactive counts fall back
    /// to the number of items actually built).
    pub fn item_count(&self) -> u64 {
        match &self.view.item_count {
            Val::Static(v) => *v,
            Val::Reactive(_) => self.visible.len() as u64,
        }
    }

    /// `(start_index, end_index)` inclusive — the flat index range of cells
    /// that should be mounted for the current scroll position + viewport.
    ///
    /// Computed from line (main-axis) bounds, then converted to flat indices.
    /// Overscan is applied in lines.
    pub fn compute_visible_range(&self, viewport_main: f64) -> (u64, u64) {
        let item_count = self.item_count();
        if item_count == 0 {
            return (0, 0);
        }
        let count = self.cross_axis_count.max(1) as u64;
        let stride = self.stride_main;
        if stride <= 0.0 {
            return (0, item_count - 1);
        }
        let total_lines = item_count.div_ceil(count);
        let scroll = self.position.pixels();
        let first_line = ((scroll / stride).floor().max(0.0) as u64).min(total_lines - 1);
        let last_line =
            (((scroll + viewport_main) / stride).floor().max(0.0) as u64).min(total_lines - 1);
        let first_line = first_line
            .saturating_sub(self.overscan)
            .min(total_lines - 1);
        let last_line = (last_line + self.overscan).min(total_lines - 1);

        let start_index = first_line * count;
        let end_index = ((last_line + 1) * count)
            .saturating_sub(1)
            .min(item_count - 1);
        (start_index, end_index)
    }

    /// The number of cells currently built into the tree.
    pub fn built_count(&self) -> usize {
        self.visible.len()
    }

    /// The lowest currently-mounted logical index, or `None` if nothing is
    /// mounted.
    pub fn first_mounted_index(&self) -> Option<u64> {
        self.visible.first().map(|(i, _)| *i)
    }

    /// The cross-axis cell count computed during the last layout (column
    /// count for a vertical grid).
    pub fn cross_axis_count(&self) -> usize {
        self.cross_axis_count
    }

    /// Reverse-map a built node id back to its logical item index.
    pub fn visible_index_of(&self, child_id: NodeId) -> Option<u64> {
        self.visible
            .iter()
            .find(|(_, id)| *id == child_id)
            .map(|(i, _)| *i)
    }

    /// Detect prop changes (axis + all sizing props + itemCount) by diffing
    /// freshly-read reactive values against cached ones, tearing down /
    /// resetting state as needed. Called at the top of `perform_layout`.
    pub(super) fn react_to_prop_changes(&mut self, cx: &mut dyn ViewCx, boa: &mut Context) {
        // Axis change: cached geometry is axis-specific, so reset.
        let new_axis = self
            .view
            .axis
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or(self.axis);
        if new_axis != self.axis {
            self.axis = new_axis;
            self.cross_axis_count = 1;
            self.cell_cross = 0.0;
            self.cell_main = 0.0;
            self.stride_main = 0.0;
        }

        // itemCount shrink: destroy items at or beyond the new count.
        let new_count = read_val(cx, &self.view.item_count, boa).unwrap_or(0);
        let current_max = self.visible.last().map(|(i, _)| *i + 1).unwrap_or(0);
        if new_count < current_max {
            let to_destroy: Vec<NodeId> = self
                .visible
                .iter()
                .filter(|(i, _)| *i >= new_count)
                .map(|&(_, id)| id)
                .collect();
            for id in to_destroy {
                cx.destroy_child(id);
            }
            self.visible.retain(|(i, _)| *i < new_count);
        }
    }

    /// Mount cells in the visible range that aren't currently built, and
    /// unmount any built cells outside it. Mutates the tree via `cx`. Sorted
    /// `visible` is preserved.
    pub fn remount(&mut self, cx: &mut dyn ViewCx, boa: &mut Context, viewport_main: f64) {
        if viewport_main <= 0.0 {
            return;
        }

        let count = self.item_count();
        if count == 0 {
            let to_destroy: Vec<NodeId> = self.visible.iter().map(|&(_, id)| id).collect();
            for id in to_destroy {
                cx.destroy_child(id);
            }
            self.visible.clear();
            return;
        }

        let (new_start, new_end) = self.compute_visible_range(viewport_main);

        // Unmount off-screen cells.
        let to_destroy: Vec<NodeId> = self
            .visible
            .iter()
            .filter(|(i, _)| *i < new_start || *i > new_end)
            .map(|&(_, id)| id)
            .collect();
        for id in to_destroy {
            cx.destroy_child(id);
        }
        self.visible
            .retain(|(i, _)| *i >= new_start && *i <= new_end);

        // If the column count changed (resize crossing a max-extent boundary),
        // the index→position mapping shifted: clear the cache so the position
        // step recomputes every offset. (The `visible` set is index-keyed, so
        // it survives a count change — only the geometry changes.)

        // Mount newly-visible cells.
        let existing: std::collections::HashSet<u64> =
            self.visible.iter().map(|(i, _)| *i).collect();
        let builder = self.view.builder.clone();
        let node_id = self.node_id;
        let mut newly_mounted: Vec<(u64, NodeId)> = Vec::new();
        for index in new_start..=new_end {
            if existing.contains(&index) {
                continue;
            }
            if let Some(spec) = build_item_spec(&builder, index, boa) {
                let item_id = spec.build(cx, boa, node_id.into());
                let next_higher = self
                    .visible
                    .iter()
                    .find(|(i, _)| *i > index)
                    .map(|(_, id)| *id);
                if let Some(ref_id) = next_higher {
                    cx.move_child_before(node_id, item_id, ref_id);
                }
                newly_mounted.push((index, item_id));
            }
        }
        if !newly_mounted.is_empty() {
            self.visible.extend(newly_mounted);
            self.visible.sort_by_key(|(i, _)| *i);
        }

        if (self.reported_start, self.reported_end) != (new_start, new_end) {
            self.reported_start = new_start;
            self.reported_end = new_end;
        }
    }
}

// ---------------------------------------------------------------------------
// Subscribe + reactive-change reaction.
// ---------------------------------------------------------------------------

impl crate::core::layout::ElementSubscribe for LazyGridElement {
    fn subscribe(&self, cx: &mut crate::core::layout::SubscribeCx) {
        if let Some(v) = self.view.axis.as_ref() {
            cx.subscribe_val(v);
        }
        cx.subscribe_val(&self.view.item_count);
        if let Some(v) = self.view.overscan.as_ref() {
            cx.subscribe_val(v);
        }
        cx.subscribe_val(&self.view.max_cross_axis_extent);
        if let Some(v) = self.view.child_aspect_ratio.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.main_axis_extent.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.cross_axis_spacing.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.main_axis_spacing.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl crate::core::view::Lifecycle for LazyGridElement {}

impl ElementTrace for LazyGridElement {
    fn trace_label(&self) -> String {
        format!(
            "axis={:?} items={} built={} cols={} offset={:.1} range={}-{}",
            self.axis,
            self.item_count(),
            self.visible.len(),
            self.cross_axis_count,
            self.position.pixels(),
            self.reported_start,
            self.reported_end,
        )
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        vec![
            ("axis", TraceValue::Str(format!("{:?}", self.axis))),
            ("itemCount", TraceValue::Num(self.item_count() as f64)),
            ("builtCount", TraceValue::Num(self.visible.len() as f64)),
            ("cols", TraceValue::Num(self.cross_axis_count as f64)),
            ("cellCross", TraceValue::Num(self.cell_cross)),
            ("cellMain", TraceValue::Num(self.cell_main)),
            ("overscan", TraceValue::Num(self.overscan as f64)),
        ]
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        vec![
            ("offset", TraceValue::Num(self.position.pixels())),
            (
                "maxScrollExtent",
                TraceValue::Num(self.position.max_scroll_extent()),
            ),
            ("rangeStart", TraceValue::Num(self.reported_start as f64)),
            ("rangeEnd", TraceValue::Num(self.reported_end as f64)),
        ]
    }
}

// ---------------------------------------------------------------------------
// Wheel handling — scroll the viewport along the main axis.
// ---------------------------------------------------------------------------

impl ElementOnWheel for LazyGridElement {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64 {
        let delta = match self.axis {
            Axis::Vertical => event.delta_y,
            Axis::Horizontal => event.delta_x,
        };

        let old_pixels = self.position.pixels();
        let overscroll = self.position.apply_scroll_delta(delta);
        let new_pixels = self.position.pixels();

        if (new_pixels - old_pixels).abs() > 0.001 {
            cx.request_paint();
        }

        overscroll
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl LazyGridView {
    /// Build a `LazyGridView` from a JS props object. Returns `None` when a
    /// required prop (`itemCount`, `maxCrossAxisExtent`, `builder`) is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let mut p = JsProps::new(props, ctx);
        let item_count = p.val::<u64>("itemCount")?;
        let max_cross_axis_extent = p.val::<f64>("maxCrossAxisExtent")?;
        let builder = p.function("builder")?;
        Some(LazyGridView {
            axis: p.val::<Axis>("axis"),
            item_count,
            overscan: p.val::<u64>("overscan"),
            max_cross_axis_extent,
            child_aspect_ratio: p.val::<f64>("childAspectRatio"),
            main_axis_extent: p.val::<f64>("mainAxisExtent"),
            cross_axis_spacing: p.val::<f64>("crossAxisSpacing"),
            main_axis_spacing: p.val::<f64>("mainAxisSpacing"),
            builder,
            query_key: p.query_key("queryKey"),
        })
    }
}

// ---------------------------------------------------------------------------
// Visible-range event payload — JS callback arguments for
// onVisibleRangeChange (LazyGridController only).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct VisibleRangeChangeEvent {
    pub(crate) start_index: u64,
    pub(crate) end_index: u64,
}

impl IntoJsArgs for VisibleRangeChangeEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![
            JsValue::from(self.start_index as f64),
            JsValue::from(self.end_index as f64),
        ]
    }
}
