use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::Axis;

use crate::core::edgy_event::EventArg;
use crate::core::element::ElementNodeId;
use crate::core::elements::{
    AnyElement, ElementOnWheel, ElementOnWheelContext, ElementTrace,
    WheelEvent,
};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

use crate::elements::lazy_list::controller::LazyListController;
use crate::elements::scroll_view::ScrollPosition;

const FALLBACK_EXTENT: f64 = 50.0;

// ---------------------------------------------------------------------------
// LazyListComponent — the user's declaration.
//
// `axis`, `itemCount`, and `overscan` are reactive (`Val<T>`). `builder` is a
// JS function `(index) => EdgyElement` captured at factory time and stored as a
// `JsFunction`. The spec uses `unsafe_empty_trace` — same risk profile as the
// old EdgyHandle holding a JsFunction: the closure is kept alive by the JS
// module scope for the lifetime of the app, so the GC always sees it via that
// root.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LazyListComponent {
    pub axis: Option<Val<Axis>>,
    pub item_count: Val<u64>,
    pub overscan: Option<Val<u64>>,
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

        // Build items eagerly up to `item_count`. Items are built BEFORE this
        // node is inserted, so each item's self-link to `id` is a no-op; we
        // explicitly link them after inserting to get a single edge per item.
        let builder = self.builder.clone();
        let mut visible: Vec<(u64, ElementNodeId)> = Vec::new();
        for index in 0..item_count {
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
                position: ScrollPosition::new(),
                child_extents: Vec::new(),
                visible,
                reported_start: 0,
                reported_end: 0,
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
// ---------------------------------------------------------------------------

pub struct LazyListElement {
    pub component: LazyListComponent,
    pub(crate) node_id: ElementNodeId,
    pub(crate) axis: Axis,
    pub(crate) overscan: u64,
    pub(crate) position: ScrollPosition,
    pub(crate) child_extents: Vec<f64>,
    /// (index, built node id) for every item currently in the tree.
    pub(crate) visible: Vec<(u64, ElementNodeId)>,
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
        match &self.component.item_count {
            Val::Static(v) => *v,
            Val::Reactive(_) => self.visible.len() as u64,
        }
    }

    /// The number of items currently built into the tree.
    pub fn built_count(&self) -> usize {
        self.visible.len()
    }

    pub fn average_extent(&self) -> f64 {
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
        let avg = self.average_extent();
        if avg <= 0.0 {
            return (0, count.saturating_sub(1));
        }
        let scroll = self.position.pixels();
        let start = ((scroll / avg).floor() as u64).min(count.saturating_sub(1));
        let end = (((scroll + viewport_main) / avg).ceil() as u64).min(count.saturating_sub(1));
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
}

// ---------------------------------------------------------------------------
// Effect — rebuild the item set when itemCount changes.
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

        if axis_dirty {
            self.axis = self
                .component
                .axis
                .as_ref()
                .and_then(|v| cx.read_val(v, boa))
                .unwrap_or(self.axis);
        }

        if !count_dirty {
            return;
        }

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
            // Build items for the newly-visible indices. The LazyListElement node is
            // in the tree during the effect, so each item's self-link succeeds
            // — no explicit link needed.
            let builder = self.component.builder.clone();
            let node_id = self.node_id;
            for index in current_max..new_count {
                let Some(spec) = build_item_spec(&builder, index, boa) else {
                    continue;
                };
                let item_id = spec.build(cx, boa, node_id);
                self.visible.push((index, item_id));
            }
        }

        cx.mark_dirty(self.node_id);
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
}

// ---------------------------------------------------------------------------
// Wheel handling — scroll the viewport along the main axis.
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
