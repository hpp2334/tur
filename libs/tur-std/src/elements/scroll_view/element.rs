use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{Axis, Brush, Size};

use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_engine::core::layout::{ElementSubscribe, SubscribeCx};
use tur_engine::core::elements::{
    AnyElement, ElementOnWheel, ElementOnWheelContext, ElementTrace,
    TraceValue, WheelEvent,
};
use crate::scroll::{ScrollController, ScrollEvent};
use tur_engine::core::view::{
    ViewCx,
    read_val,
    extract_view, val_from_js, Lifecycle, PropValue, View, Val,
};

use super::scroll_position::ScrollPosition;

// ---------------------------------------------------------------------------
// ScrollViewView — the user's declaration. Pure Rust, no JsValues.
//
// `axis`, `padding`, and `color` are reactive (`Val<T>`).
// `controller` is a JS `ScrollController` opaque — parsed eagerly at factory
// time (not reactive).  `child` is required (the scrollable content).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ScrollViewView {
    pub(crate) axis: Option<Val<Axis>>,
    pub(crate) padding: Option<Val<f64>>,
    pub(crate) color: Option<Val<Brush>>,
    /// JS `ScrollController` opaque — parsed eagerly (not reactive).
    pub(crate) controller: Option<JsObject>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) child: Rc<dyn View>,
}

impl View for ScrollViewView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        // Resolve axis eagerly — the wheel handler and controller-metric
        // updates need it at event time where no store/Context is available.
        let axis = self
            .axis
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or(Axis::Vertical);

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::with_wheel(ScrollViewElement {
                view: self.clone(),
                axis,
                position: ScrollPosition::new(),
                painting: ScrollViewPainting::default(),
            })
            .with_callbacks(),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        // Bind the controller to this node so `jumpTo` (and drag-driven
        // `ScrollTo` events from a sibling Scrollbar) can locate this element.
        if let Some(ctrl_obj) = &self.controller {
            if let Some(mut ctrl) = ctrl_obj.downcast_mut::<ScrollController>() {
                ctrl.bound_node = Some(id);
                ctrl.element_tree = Some(cx.node_tree());
                ctrl.mutation_queue = Some(cx.mutation_queue());
                ctrl.dirty_flag = Some(cx.dirty());
            }
        }
        let _child_id = self.child.build(cx, boa, id.into());
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// ScrollViewElement — the built element. Holds its spec, the eagerly-resolved axis,
// and the mutable scroll position.
// ---------------------------------------------------------------------------

/// Resolved paint props (filled during layout). Paint reads these directly.
#[derive(Default, Clone)]
pub struct ScrollViewPainting {
    pub(crate) color: Option<Brush>,
}

pub struct ScrollViewElement {
    pub(crate) view: ScrollViewView,
    pub(crate) axis: Axis,
    pub(crate) position: ScrollPosition,
    pub(crate) painting: ScrollViewPainting,
}

impl ScrollViewElement {
    pub fn scroll_offset(&self) -> f64 {
        self.position.pixels()
    }

    /// Maximum scrollable offset along the scroll axis (content - viewport,
    /// clamped to be non-negative).
    pub fn max_scroll_extent(&self) -> f64 {
        self.position.max_scroll_extent()
    }

    pub fn content_size(&self) -> Size {
        self.position.content_size()
    }

    pub fn viewport_size(&self) -> Size {
        self.position.viewport_size()
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    pub fn update_controller_metrics(&mut self) {
        let Some(ref ctrl_obj) = self.view.controller else { return };
        let Some(mut ctrl) = ctrl_obj.downcast_mut::<ScrollController>() else {
            return;
        };
        let vp = self.position.viewport_size();
        let dim = match self.axis {
            Axis::Vertical => vp.height,
            Axis::Horizontal => vp.width,
        };
        ctrl.offset = self.position.pixels();
        ctrl.max_scroll_extent = self.position.max_scroll_extent();
        ctrl.viewport_dimension = dim;
    }

    pub fn apply_pending_initial_offset(&mut self) {
        let Some(ref ctrl_obj) = self.view.controller else { return };
        let Some(mut ctrl) = ctrl_obj.downcast_mut::<ScrollController>() else {
            return;
        };
        let Some(initial) = ctrl.pending_initial_offset.take() else {
            return;
        };
        let clamped = initial.clamp(0.0, self.position.max_scroll_extent());
        self.position.correct_pixels(clamped);
        ctrl.offset = clamped;
    }
}

impl Lifecycle for ScrollViewElement {}

impl ElementSubscribe for ScrollViewElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.padding.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.color.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for ScrollViewElement {
    fn trace_label(&self) -> String {
        let vp = self.viewport_size();
        let ct = self.content_size();
        format!(
            "axis={:?} offset={:.1} viewport=({:.1},{:.1}) content=({:.1},{:.1})",
            self.axis,
            self.position.pixels(),
            vp.width,
            vp.height,
            ct.width,
            ct.height,
        )
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        vec![("axis", TraceValue::Str(format!("{:?}", self.axis)))]
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        let vp = self.viewport_size();
        let ct = self.content_size();
        vec![
            ("offset", TraceValue::Num(self.position.pixels())),
            ("maxScrollExtent", TraceValue::Num(self.position.max_scroll_extent())),
            ("viewportWidth", TraceValue::Num(vp.width)),
            ("viewportHeight", TraceValue::Num(vp.height)),
            ("contentWidth", TraceValue::Num(ct.width)),
            ("contentHeight", TraceValue::Num(ct.height)),
        ]
    }
}

impl ElementOnWheel for ScrollViewElement {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64 {
        let delta = match self.axis {
            Axis::Vertical => event.delta_y,
            Axis::Horizontal => event.delta_x,
        };

        let old_pixels = self.position.pixels();
        let overscroll = self.position.apply_scroll_delta(delta);
        let new_pixels = self.position.pixels();

        if (new_pixels - old_pixels).abs() > 0.001 {
            self.update_controller_metrics();
            if let Some(ref ctrl_obj) = self.view.controller {
                if let Some(ctrl) = ctrl_obj.downcast_ref::<ScrollController>() {
                    if let Some(m) = ctrl.on_scroll {
                        cx.push_event(
                            m,
                            ScrollEvent {
                                offset: ctrl.offset,
                                max_extent: ctrl.max_scroll_extent,
                                viewport_dimension: ctrl.viewport_dimension,
                            },
                        );
                    }
                }
            }
            cx.request_redraw();
        }

        overscroll
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract a `Vec<String>` prop (queryKey) — parsed eagerly.
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

/// Extract the controller `JsObject` — parsed eagerly (not reactive).
fn prop_controller(props: &JsObject, key: &str, ctx: &mut Context) -> Option<JsObject> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    v.as_object()
}

/// Extract the single child spec from a JS props object.
fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn View>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_view(&v)
}

impl ScrollViewView {
    /// Build a `ScrollViewView` from a JS props object. Returns `None` when
    /// the required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let child = prop_child(props, "child", ctx)?;
        Some(ScrollViewView {
            axis: prop_val::<Axis>(props, "axis", ctx),
            padding: prop_val::<f64>(props, "padding", ctx),
            color: prop_val::<Brush>(props, "color", ctx),
            controller: prop_controller(props, "controller", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            child,
        })
    }
}
