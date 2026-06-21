use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::Axis;

use crate::core::edgy_event::EventArg;
use crate::core::element::ElementNodeId;
use crate::core::elements::{
    AnyElement, ElementOnWheel, ElementOnWheelContext, ElementTrace,
    TraceValue, WheelEvent,
};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

use crate::elements::lazy_list::controller::LazyListController;
use crate::elements::scroll_view::ScrollPosition;

const FALLBACK_EXTENT: f64 = 50.0;
/// The default number of items built up-front when no viewport information
/// is available yet (i.e. during the initial `build()` before the first
/// layout pass). Once layout runs and we know the real viewport size, the
/// range is recomputed and unmounted items are pruned.
const INITIAL_BUILD_COUNT: u64 = 20;

// ---------------------------------------------------------------------------
// LazyListComponent — the user's declaration.
//
// `axis`, `itemCount`, `overscan`, and `itemExtent` are reactive (`Val<T>`).
// `builder` is a JS function `(index) => EdgyElement` captured at factory
// time and stored as a `JsFunction`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LazyListComponent {
    pub axis: Option<Val<Axis>>,
    pub item_count: Val<u64>,
    pub overscan: Option<Val<u64>>,
    /// Optional fixed extent (size along the main axis) for every item.
    /// When provided, the visible-range math is exact and we never need to
    /// measure items off-screen to know the total content length. When
    /// absent, the average of measured children is used as a fallback.
    pub item_extent: Option<Val<f64>>,
    pub builder: JsFunction,
    pub query_key: Option<Vec<String>>,
}

impl Component for LazyListComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();

        // Resolve the eager props needed by the element up-front.
        let axis = self
            .axis
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or(Axis::Vertical);
        let item_count = cx.read_val(&self.item_count, boa).unwrap_or(0);
        let overscan = self
            .overscan
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or(3);
        let item_extent = self
            .item_extent
            .as_ref()
            .and_then(|v| cx.read_val(v, boa));

        // Build only the first INITIAL_BUILD_COUNT items (or fewer if
        // item_count is smaller). After the first layout, the remount pass
        // will adjust the mounted set to match the actual viewport.
        let initial_count = item_count.min(INITIAL_BUILD_COUNT);
        let builder = self.builder.clone();
        let mut visible: Vec<(u64, ElementNodeId)> = Vec::new();
        for index in 0..initial_count {
            let Some(spec) = build_item_spec(&builder, index, boa) else {
                continue;
            };
            let item_id = spec.build(cx, boa, id);
            visible.push((index, item_id));
        }
        let item_ids: Vec<ElementNodeId> = visible.iter().map(|&(_, id)| id).collect();

        cx.insert_node(
            id,
            AnyElement::with_wheel(LazyListElement {
                component: self.clone(),
                node_id: id,
                axis,
                overscan,
                item_extent,
                position: ScrollPosition::new(),
                child_extents: Vec::new(),
                visible,
                reported_start: 0,
                reported_end: 0,
                remount_requested: true,
                last_viewport_main: 0.0,
            })
            .with_callbacks(),
            boa,
        );

        for item_id in item_ids {
            cx.link_child(id, item_id);
        }
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id);
        id
    }
}

/// Invoke the JS builder closure for `index`, returning the produced spec.
fn build_item_spec(
    builder: &JsFunction,
    index: u64,
    boa: &mut Context,
) -> Option<Rc<dyn Component>> {
    let result = builder
        .call(&JsValue::undefined(), &[JsValue::from(index as f64)], boa)
        .ok()?;
    extract_component(&result)
}

// ---------------------------------------------------------------------------
// LazyListElement — the built element. A scroll container that lays its built items
// out sequentially along the main axis, clips to the viewport, and offsets
// children by the current scroll position.
//
// Virtualization: only items inside [start, end] (computed from scroll
// position + viewport size + overscan) are mounted. The wheel handler sets
// `remount_requested=true`; a flush pass then mounts/unmounts items to
// match the new range.
// ---------------------------------------------------------------------------

pub struct LazyListElement {
    pub component: LazyListComponent,
    pub(crate) node_id: ElementNodeId,
    pub(crate) axis: Axis,
    pub(crate) overscan: u64,
    pub(crate) item_extent: Option<f64>,
    pub(crate) position: ScrollPosition,
    pub(crate) child_extents: Vec<f64>,
    /// (index, built node id) for every item currently in the tree. Kept
    /// sorted by index.
    pub(crate) visible: Vec<(u64, ElementNodeId)>,
    pub(crate) reported_start: u64,
    pub(crate) reported_end: u64,
    /// Set by `on_wheel` when the scroll offset shifts enough to possibly
    /// change the visible range. Consumed by `process_remount`.
    pub(crate) remount_requested: bool,
    /// Last viewport main-axis size seen during layout. Used by
    /// `compute_visible_range` when the wheel fires between layouts.
    pub(crate) last_viewport_main: f64,
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
        match &self.component.item_count {
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
        let start = ((scroll / extent).floor() as i64).max(0) as u64;
        let end = (((scroll + viewport_main) / extent).ceil() as i64).max(0) as u64;
        let start = start.min(count.saturating_sub(1));
        let end = end.min(count.saturating_sub(1));
        let start = start.saturating_sub(self.overscan);
        let end = (end + self.overscan).min(count.saturating_sub(1));
        (start, end)
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

    /// Mount items in `[start, end]` that aren't currently built, and
    /// unmount any built items outside that range. Mutates the tree via
    /// `cx`. Sorted visible is preserved.
    ///
    /// This is called from `process_pending_remounts` in the flush loop
    /// — it has full WidgetCx + Context access, which `on_wheel` does not.
    pub fn process_remount(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        viewport_main: f64,
    ) {
        // Defer remount until we have a real viewport size. Layout writes
        // the viewport into `last_viewport_main`; until then we keep the
        // initial set mounted so the first paint isn't blank.
        if viewport_main <= 0.0 {
            return;
        }
        self.remount_requested = false;
        self.last_viewport_main = viewport_main;

        let count = self.item_count();
        if count == 0 {
            // Tear down any stragglers.
            let to_destroy: Vec<ElementNodeId> =
                self.visible.iter().map(|&(_, id)| id).collect();
            for id in to_destroy {
                cx.destroy_subtree(id);
            }
            self.visible.clear();
            cx.mark_dirty(self.node_id);
            return;
        }

        let (new_start, new_end) = self.compute_visible_range(viewport_main);

        // Unmount off-screen items.
        let to_destroy: Vec<ElementNodeId> = self
            .visible
            .iter()
            .filter(|(i, _)| *i < new_start || *i > new_end)
            .map(|&(_, id)| id)
            .collect();
        let mut did_change = !to_destroy.is_empty();
        for id in to_destroy {
            cx.destroy_subtree(id);
        }
        self.visible.retain(|(i, _)| *i >= new_start && *i <= new_end);

        // Mount newly-visible items.
        let existing: std::collections::HashSet<u64> =
            self.visible.iter().map(|(i, _)| *i).collect();
        let builder = self.component.builder.clone();
        let node_id = self.node_id;
        let mut newly_mounted: Vec<(u64, ElementNodeId)> = Vec::new();
        for index in new_start..=new_end {
            if existing.contains(&index) {
                continue;
            }
            if let Some(spec) = build_item_spec(&builder, index, boa) {
                let item_id = spec.build(cx, boa, node_id);
                newly_mounted.push((index, item_id));
            }
        }
        if !newly_mounted.is_empty() {
            did_change = true;
            self.visible.extend(newly_mounted);
            self.visible.sort_by_key(|(i, _)| *i);
        }

        if did_change {
            cx.mark_dirty(self.node_id);
        }

        // Report visible-range change.
        if (self.reported_start, self.reported_end) != (new_start, new_end) {
            self.reported_start = new_start;
            self.reported_end = new_end;
        }
    }
}

// ---------------------------------------------------------------------------
// Effect — rebuild the item set when itemCount or axis changes. Also runs
// the remount logic if requested (the wheel handler sets the flag and the
// flush loop triggers this effect).
// ---------------------------------------------------------------------------

impl Effect for LazyListElement {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        let count_dirty = self.component.item_count.is_dirty(dirties);
        let axis_dirty = self.component.axis.as_ref().is_some_and(|v| v.is_dirty(dirties));
        let extent_dirty = self.component.item_extent.as_ref().is_some_and(|v| v.is_dirty(dirties));

        if axis_dirty {
            self.axis = self
                .component
                .axis
                .as_ref()
                .and_then(|v| cx.read_val(v, boa))
                .unwrap_or(self.axis);
        }

        if extent_dirty {
            self.item_extent = self
                .component
                .item_extent
                .as_ref()
                .and_then(|v| cx.read_val(v, boa));
        }

        if count_dirty {
            let new_count = cx.read_val(&self.component.item_count, boa).unwrap_or(0);
            let current_max = self.visible.last().map(|(i, _)| *i + 1).unwrap_or(0);

            if new_count < current_max {
                // Destroy items whose index is at or beyond the new count.
                let to_destroy: Vec<ElementNodeId> = self
                    .visible
                    .iter()
                    .filter(|(i, _)| *i >= new_count)
                    .map(|&(_, id)| id)
                    .collect();
                for id in to_destroy {
                    cx.destroy_subtree(id);
                }
                self.visible.retain(|(i, _)| *i < new_count);
            } else if new_count > current_max {
                // Only build the items that fall inside the current visible
                // range. Items beyond it will be built lazily by the remount
                // pass as the user scrolls.
                let vp = self.last_viewport_main;
                let (start, end) = if vp > 0.0 {
                    self.compute_visible_range(vp)
                } else {
                    (0, new_count.min(INITIAL_BUILD_COUNT).saturating_sub(1))
                };
                let builder = self.component.builder.clone();
                let node_id = self.node_id;
                for index in current_max.max(start).max(current_max)..=end.max(current_max).min(new_count.saturating_sub(1)) {
                    if index < current_max {
                        continue;
                    }
                    let Some(spec) = build_item_spec(&builder, index, boa) else {
                        continue;
                    };
                    let item_id = spec.build(cx, boa, node_id);
                    self.visible.push((index, item_id));
                }
                self.visible.sort_by_key(|(i, _)| *i);
            }

            cx.mark_dirty(self.node_id);
        }
    }
}

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
            self.remount_requested = true;
            cx.request_redraw();
        }

        overscroll
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_query_key(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<String>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(val) = arr.at(i as i64, ctx) {
            if let Some(s) = val.as_string() {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn prop_builder(props: &JsObject, key: &str, ctx: &mut Context) -> Option<JsFunction> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    v.as_object().and_then(JsFunction::from_object)
}

impl LazyListComponent {
    /// Build a `LazyListComponent` from a JS props object. Returns `None` when a
    /// required prop (`itemCount`, `builder`) is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let item_count = prop_val::<u64>(props, "itemCount", ctx)?;
        let builder = prop_builder(props, "builder", ctx)?;
        Some(LazyListComponent {
            axis: prop_val::<Axis>(props, "axis", ctx),
            item_count,
            overscan: prop_val::<u64>(props, "overscan", ctx),
            item_extent: prop_val::<f64>(props, "itemExtent", ctx),
            builder,
            query_key: prop_query_key(props, "queryKey", ctx),
        })
    }
}

// ---------------------------------------------------------------------------
// Visible-range event payload — JS callback arguments for
// onVisibleRangeChange (LazyListController only).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct VisibleRangeChangeEvent {
    pub start_index: u64,
    pub end_index: u64,
}

impl EventArg for VisibleRangeChangeEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![
            JsValue::from(self.start_index as f64),
            JsValue::from(self.end_index as f64),
        ]
    }
}
