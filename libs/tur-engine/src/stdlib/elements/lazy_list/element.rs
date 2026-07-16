use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::Axis;

use crate::core::bridge::JsProps;
use crate::core::edgy_event::IntoJsArgs;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{
    AnyElement, ElementOnWheel, ElementOnWheelContext, ElementTrace,
    TraceValue, WheelEvent,
};
use crate::core::view::{ViewCx, read_val, Val, View, extract_view};

use crate::stdlib::elements::lazy_list::controller::LazyListController;
use crate::stdlib::elements::scroll_view::ScrollPosition;

const FALLBACK_EXTENT: f64 = 50.0;
/// The default number of items built up-front when no viewport information
/// is available yet (i.e. during the initial `build()` before the first
/// layout pass). Once layout runs and we know the real viewport size, the
/// range is recomputed and unmounted items are pruned.
const INITIAL_BUILD_COUNT: u64 = 20;

// ---------------------------------------------------------------------------
// LazyListView — the user's declaration.
//
// `axis`, `itemCount`, `overscan`, and `itemExtent` are reactive (`Val<T>`).
// `builder` is a JS function `(index) => EdgyElement` captured at factory
// time and stored as a `JsFunction`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LazyListView {
    pub(crate) axis: Option<Val<Axis>>,
    pub(crate) item_count: Val<u64>,
    pub(crate) overscan: Option<Val<u64>>,
    /// Optional fixed extent (size along the main axis) for every item.
    /// When provided, the visible-range math is exact and we never need to
    /// measure items off-screen to know the total content length. When
    /// absent, the average of measured children is used as a fallback.
    pub(crate) item_extent: Option<Val<f64>>,
    pub(crate) builder: JsFunction,
    pub(crate) query_key: Option<Vec<String>>,
}

impl View for LazyListView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());

        // Resolve the eager props needed by the element up-front.
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
        let item_extent = self
            .item_extent
            .as_ref()
            .and_then(|v| read_val(cx, v, boa));

        // Build only the first INITIAL_BUILD_COUNT items (or fewer if
        // item_count is smaller). After the first layout, the remount pass
        // will adjust the mounted set to match the actual viewport.
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
            AnyElement::with_wheel(LazyListElement {
                view: self.clone(),
                node_id: id,
                axis,
                overscan,
                item_extent,
                position: ScrollPosition::new(),
                child_extents: Vec::new(),
                extent_cache: std::collections::BTreeMap::new(),
                visible,
                first_mounted_index: 0,
                first_mounted_offset: 0.0,
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
fn build_item_spec(
    builder: &JsFunction,
    index: u64,
    boa: &mut Context,
) -> Option<Rc<dyn View>> {
    let result = builder
        .call(&JsValue::undefined(), &[JsValue::from(index as f64)], boa)
        .ok()?;
    extract_view(&result)
}

// ---------------------------------------------------------------------------
// LazyListElement — the built element. A scroll container that lays its built items
// out sequentially along the main axis, clips to the viewport, and offsets
// children by the current scroll position.
//
// Virtualization: only items inside [start, end] (computed from scroll
// position + viewport size + overscan) are mounted. Remount happens inside
// `perform_layout` (via `LayoutViewCx`), using the real viewport from
// constraints, so newly-visible items mount/unmount in the same pass that
// measures them.
// ---------------------------------------------------------------------------

pub struct LazyListElement {
    pub(crate) view: LazyListView,
    pub(crate) node_id: ElementNodeId,
    pub(crate) axis: Axis,
    pub(crate) overscan: u64,
    pub(crate) item_extent: Option<f64>,
    pub(crate) position: ScrollPosition,
    pub(crate) child_extents: Vec<f64>,
    /// Per-index persistent cache of measured main-axis extents. Survives
    /// across layouts (unlike `child_extents`, which is cleared and refilled
    /// every layout). Used to compute cumulative offsets so variable-height
    /// items are positioned at the actual running sum of previous heights
    /// rather than `index * averageExtent` — which caused overlaps when
    /// items had different sizes. Cleared on axis/itemExtent/itemCount
    /// changes via `react_to_prop_changes` in `perform_layout`.
    pub(crate) extent_cache: std::collections::BTreeMap<u64, f64>,
    /// (index, built node id) for every item currently in the tree. Kept
    /// sorted by index.
    pub(crate) visible: Vec<(u64, NodeId)>,
    /// Logical index of the first mounted item (`visible[0].0`, cached for
    /// O(1) access). Together with `first_mounted_offset`, forms the
    /// persistent anchor for layout positioning: each mounted child's
    /// content-space offset is `first_mounted_offset` + sum of extents
    /// between it and the first mounted. Maintained incrementally in
    /// `remount` so positioning is O(visible_count) regardless of
    /// scroll depth (no `cumulative_offset` walk from 0).
    pub(crate) first_mounted_index: u64,
    /// Content-space offset of the first mounted item's top edge.
    /// `perform_layout`'s position step walks forward from this anchor, setting
    /// each child's offset to the running sum. Synced in `remount`
    /// as the leading edge shifts (delta-update: O(items added/removed at
    /// the leading edge), not O(first_mounted_index)).
    pub(crate) first_mounted_offset: f64,
    pub(crate) reported_start: u64,
    pub(crate) reported_end: u64,
}

impl LazyListElement {
    pub fn scroll_offset(&self) -> f64 {
        self.position.pixels()
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// The declared item count (static value only; reactive counts fall back
    /// to the number of items actually built, which is kept in sync by the
    /// effect).
    pub fn item_count(&self) -> u64 {
        match &self.view.item_count {
            Val::Static(v) => *v,
            Val::Reactive(_) => self.visible.len() as u64,
        }
    }

    /// The number of items currently built into the tree.
    pub fn built_count(&self) -> usize {
        self.visible.len()
    }

    /// The lowest currently-mounted index, or `None` if nothing is mounted.
    pub fn first_mounted_index(&self) -> Option<u64> {
        self.visible.first().map(|(i, _)| *i)
    }

    pub fn average_extent(&self) -> f64 {
        if let Some(ext) = self.item_extent {
            return ext;
        }
        let n = self.child_extents.len();
        if n == 0 {
            return FALLBACK_EXTENT;
        }
        let sum: f64 = self.child_extents.iter().sum();
        let avg = sum / n as f64;
        if avg <= 0.0 { FALLBACK_EXTENT } else { avg }
    }

    /// The cumulative main-axis offset of item `index`'s top edge (i.e. the
    /// sum of extents of items `0..index`). Uses cached measurements for
    /// items that have been mounted; falls back to `average_extent()` for
    /// items that haven't. This is what `perform_layout`'s position step uses to
    /// place each child at its true running-sum offset rather than
    /// `index * averageExtent`, which produced overlaps for variable-height
    /// lists.
    pub fn cumulative_offset(&self, index: u64) -> f64 {
        let avg = self.average_extent();
        let mut sum = 0.0;
        for i in 0..index {
            sum += self.extent_cache.get(&i).copied().unwrap_or(avg);
        }
        sum
    }

    pub fn compute_visible_range(&self, viewport_main: f64) -> (u64, u64) {
        let count = self.item_count();
        if count == 0 {
            return (0, 0);
        }
        let extent = self.average_extent();
        if extent <= 0.0 {
            return (0, count.saturating_sub(1));
        }
        let scroll = self.position.pixels();
        let target_bottom = scroll + viewport_main;
        let count_m1 = count.saturating_sub(1);

        // Walk item-by-item, accumulating offsets. The first item whose
        // bottom (`offset + extent`) exceeds `scroll` is the visible range
        // start; the last item whose top (`offset`) is below `target_bottom`
        // is the end. Using cached extents for measured items makes this
        // exact for the mounted region; the avg fallback approximates the
        // rest. Bug 7 in the design doc is now properly fixed.
        let mut offset = 0.0;
        let mut start: Option<u64> = None;
        let mut end: u64 = 0;
        for i in 0..count {
            let ext = self.extent_cache.get(&i).copied().unwrap_or(extent);
            if start.is_none() && offset + ext > scroll {
                start = Some(i);
            }
            if offset < target_bottom {
                end = i;
            } else {
                break;
            }
            offset += ext;
        }
        let start = start.unwrap_or(0);
        let start = start.saturating_sub(self.overscan).min(count_m1);
        let end = (end + self.overscan).min(count_m1);
        (start, end)
    }

    /// Reverse-map a built node id back to its logical item index. Returns
    /// `None` if the child isn't currently mounted. Used by layout to
    /// position children by their logical index (which is stable across
    /// scroll-driven mount/unmount) rather than by their position in the
    /// parent's children vector (which can be scrambled when items mount
    /// out of order — see `remount`).
    pub fn visible_index_of(&self, child_id: NodeId) -> Option<u64> {
        self.visible
            .iter()
            .find(|(_, id)| *id == child_id)
            .map(|(i, _)| *i)
    }

    #[allow(dead_code)]
    pub(crate) fn update_controller_metrics(&mut self, ctrl: &mut LazyListController) {
        let vp = self.position.viewport_size();
        let dim = match self.axis {
            Axis::Vertical => vp.height,
            Axis::Horizontal => vp.width,
        };
        ctrl.offset = self.position.pixels();
        ctrl.max_scroll_extent = self.position.max_scroll_extent();
        ctrl.viewport_dimension = dim;
    }

    /// Detect axis / itemExtent / itemCount changes by diffing freshly-read
    /// reactive values against the cached ones on `self`, and tear down /
    /// reset state accordingly. Idempotent: once the cached value matches the
    /// read value, subsequent passes are no-ops. Called at the top of
    /// `perform_layout` (with a `LayoutViewCx` so tree mutations work);
    /// replaces the former pre-layout `Effect` handler.
    pub(super) fn react_to_prop_changes(&mut self, cx: &mut dyn ViewCx, boa: &mut Context) {
        // Axis change: cached extents are axis-specific, so invalidate them
        // and reset the positioning anchor.
        let new_axis = self
            .view
            .axis
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or(self.axis);
        if new_axis != self.axis {
            self.axis = new_axis;
            self.extent_cache.clear();
            self.first_mounted_index = 0;
            self.first_mounted_offset = 0.0;
        }

        // itemExtent change: invalidate cached measurements and the anchor.
        let new_extent = self
            .view
            .item_extent
            .as_ref()
            .and_then(|v| read_val(cx, v, boa));
        if new_extent != self.item_extent {
            self.item_extent = new_extent;
            self.extent_cache.clear();
            self.first_mounted_index = 0;
            self.first_mounted_offset = 0.0;
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
            self.extent_cache.retain(|i, _| *i < new_count);
            // If the leading edge got chopped off, recompute the anchor.
            let new_first = self.visible.first().map(|(i, _)| *i).unwrap_or(0);
            if new_first != self.first_mounted_index {
                self.first_mounted_index = new_first;
                self.first_mounted_offset = self.cumulative_offset(new_first);
            }
        }
        // itemCount grew: don't build eagerly — `remount` mounts the
        // newly-in-range items using the real viewport.
    }

    /// Mount items in `[start, end]` that aren't currently built, and
    /// unmount any built items outside that range. Mutates the tree via
    /// `cx`. Sorted `visible` is preserved.
    ///
    /// Called from `perform_layout` with the **real** viewport (from
    /// constraints) via a `LayoutViewCx` — so remount runs during layout,
    /// not as a separate pre-layout pass.
    pub fn remount(
        &mut self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        viewport_main: f64,
    ) {
        // Defer remount until we have a real viewport size. Until then keep
        // the initial set mounted so the first paint isn't blank.
        if viewport_main <= 0.0 {
            return;
        }

        let count = self.item_count();
        if count == 0 {
            // Tear down any stragglers.
            let to_destroy: Vec<NodeId> =
                self.visible.iter().map(|&(_, id)| id).collect();
            for id in to_destroy {
                cx.destroy_child(id);
            }
            self.visible.clear();
            return;
        }

        let (new_start, new_end) = self.compute_visible_range(viewport_main);

        // Unmount off-screen items.
        let to_destroy: Vec<NodeId> = self
            .visible
            .iter()
            .filter(|(i, _)| *i < new_start || *i > new_end)
            .map(|&(_, id)| id)
            .collect();
        for id in to_destroy {
            cx.destroy_child(id);
        }
        self.visible.retain(|(i, _)| *i >= new_start && *i <= new_end);

        // Mount newly-visible items.
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
                // Ensure the tree children vector stays ordered by logical
                // index. `spec.build` already appended the new child to the
                // end of `node.children`; if there's an existing mounted
                // item with a larger index, *move* (not re-add) the new
                // child before it. The cheap path (new index > all
                // existing) leaves the append in place.
                //
                // Using `link_child_before` here would double-add the id
                // and crash layout; `move_child_before` removes the
                // existing slot first, then re-inserts.
                let next_higher = self.visible.iter().find(|(i, _)| *i > index).map(|(_, id)| *id);
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

        // Sync the positioning anchor (`first_mounted_index` +
        // `first_mounted_offset`) to the new leading edge. As items are
        // unmounted from the top, advance the offset by their (cached)
        // extents; as new items mount at the top, retreat the offset. This
        // keeps the position step O(visible_count) regardless of
        // scroll depth — no `cumulative_offset` walk from 0 needed.
        let avg = self.average_extent();
        let new_first = self.visible.first().map(|(i, _)| *i).unwrap_or(0);
        // Leading edge moved DOWN (items unmounted from top): advance offset.
        while self.first_mounted_index < new_first {
            let ext = self
                .extent_cache
                .get(&self.first_mounted_index)
                .copied()
                .unwrap_or(avg);
            self.first_mounted_offset += ext;
            self.first_mounted_index += 1;
        }
        // Leading edge moved UP (items mounted at top): retreat offset.
        while self.first_mounted_index > new_first {
            self.first_mounted_index -= 1;
            let ext = self
                .extent_cache
                .get(&self.first_mounted_index)
                .copied()
                .unwrap_or(avg);
            self.first_mounted_offset -= ext;
        }

        // Report visible-range change.
        if (self.reported_start, self.reported_end) != (new_start, new_end) {
            self.reported_start = new_start;
            self.reported_end = new_end;
        }
    }
}

// ---------------------------------------------------------------------------
// Subscribe + reactive-change reaction.
//
// `subscribe` declares this element's reactive deps so a prop change marks
// it dirty and re-runs `perform_layout`. `react_to_prop_changes` (called at
// the top of `perform_layout`) then detects axis / itemExtent / itemCount
// changes by diffing freshly-read values against the cached ones on `self`,
// and tears down / resets state accordingly. This replaces the former
// pre-layout `Effect` handler.
// ---------------------------------------------------------------------------

impl crate::core::layout::ElementSubscribe for LazyListElement {
    fn subscribe(&self, cx: &mut crate::core::layout::SubscribeCx) {
        if let Some(v) = self.view.axis.as_ref() {
            cx.subscribe_val(v);
        }
        cx.subscribe_val(&self.view.item_count);
        if let Some(v) = self.view.item_extent.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.overscan.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

// LazyList has no lifecycle hooks: its reactive handling lives in
// `react_to_prop_changes` (called from `perform_layout`). The default no-op
// `Lifecycle` impl satisfies the `AnyElement` bound.
impl crate::core::view::Lifecycle for LazyListElement {}

impl ElementTrace for LazyListElement {
    fn trace_label(&self) -> String {
        format!(
            "axis={:?} items={} built={} offset={:.1} range={}-{}",
            self.axis,
            self.item_count(),
            self.visible.len(),
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
            ("overscan", TraceValue::Num(self.overscan as f64)),
        ]
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        vec![
            ("offset", TraceValue::Num(self.position.pixels())),
            ("maxScrollExtent", TraceValue::Num(self.position.max_scroll_extent())),
            ("rangeStart", TraceValue::Num(self.reported_start as f64)),
            ("rangeEnd", TraceValue::Num(self.reported_end as f64)),
        ]
    }
}

// ---------------------------------------------------------------------------
// Wheel handling — scroll the viewport along the main axis and flag a
// remount. The actual mount/unmount happens in the next flush pass (which
// has Context access to call the JS builder).
// ---------------------------------------------------------------------------

impl ElementOnWheel for LazyListElement {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64 {
        let delta = match self.axis {
            Axis::Vertical => event.delta_y,
            Axis::Horizontal => event.delta_x,
        };

        let old_pixels = self.position.pixels();
        let overscroll = self.position.apply_scroll_delta(delta);
        let new_pixels = self.position.pixels();

        if (new_pixels - old_pixels).abs() > 0.001 {
            // No flag to set: the wheel handler's `mark_dirty` (in
            // `dispatch_wheel`) makes the next `perform_layout` re-run, and
            // its remount step uses the new `position.pixels()` with the real
            // viewport to adjust the mounted set.
            cx.request_redraw();
        }

        overscroll
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl LazyListView {
    /// Build a `LazyListView` from a JS props object. Returns `None` when a
    /// required prop (`itemCount`, `builder`) is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let mut p = JsProps::new(props, ctx);
        let item_count = p.val::<u64>("itemCount")?;
        let builder = p.function("builder")?;
        Some(LazyListView {
            axis: p.val::<Axis>("axis"),
            item_count,
            overscan: p.val::<u64>("overscan"),
            item_extent: p.val::<f64>("itemExtent"),
            builder,
            query_key: p.query_key("queryKey"),
        })
    }
}

// ---------------------------------------------------------------------------
// Visible-range event payload — JS callback arguments for
// onVisibleRangeChange (LazyListController only).
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
