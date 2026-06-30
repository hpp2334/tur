use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{Brush, Size};

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{
    AnyElement, ComposedGestureEvent, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementTrace, TraceValue,
};
use crate::core::scroll::ScrollController;
use crate::core::view::{
    ViewCx,
    val_from_js, Effect, PropValue, View, Val,
};

/// Minimum thumb height so it stays grabbable even for very tall content.
pub(crate) const MIN_THUMB: f64 = 24.0;
/// Default track thickness (width for a vertical scrollbar).
pub(crate) const DEFAULT_THICKNESS: f64 = 10.0;

// ---------------------------------------------------------------------------
// ScrollbarView — the user's declaration. Pure Rust except for the
// opaque `controller` (a `ScrollController` class instance shared with a
// `ScrollView`). `color`, `thumbRadius`, `trackColor` and `thickness` are
// reactive (`Val<T>`).
//
// Vertical only (the editor use case). Horizontal support can be added later.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ScrollbarView {
    /// Shared `ScrollController` — provides offset/maxExtent/viewport and the
    /// bound scroll-view node id (for `ScrollTo` requests during drag).
    pub controller: Option<JsObject>,
    pub color: Option<Val<Brush>>,
    pub track_color: Option<Val<Brush>>,
    pub thumb_radius: Option<Val<f64>>,
    /// Track thickness (width for a vertical scrollbar). Defaults to 10.
    pub thickness: Option<Val<f64>>,
    pub query_key: Option<Vec<String>>,
}

#[derive(Clone, Copy)]
struct DragState {
    /// Pointer y (local) at drag start.
    start_y: f64,
    /// Scroll offset the drag started from (after the initial click-jump, or
    /// the live offset if the click landed on the thumb).
    start_offset: f64,
}

/// Resolved paint props (filled during layout). Paint reads these directly.
#[derive(Default, Clone)]
pub struct ScrollbarPainting {
    pub track_color: Option<Brush>,
    pub color: Option<Brush>,
    pub thumb_radius: Option<f64>,
}

pub struct ScrollbarElement {
    pub view: ScrollbarView,
    /// Last computed track size — used by the drag handler for offset math.
    pub(crate) cached_track: Size,
    /// Resolved paint props — filled in `perform_layout`.
    pub(crate) painting: ScrollbarPainting,
    /// `Some` while a drag is in progress; cleared on the next pointer-up.
    drag: Option<DragState>,
}

impl View for ScrollbarView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::with_gesture_and_focus(ScrollbarElement {
                view: self.clone(),
                cached_track: Size::ZERO,
                painting: ScrollbarPainting::default(),
                drag: None,
            })
            .with_callbacks(),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

impl ScrollbarElement {
    /// Read the bound controller's live metrics: `(node_id, offset,
    /// max_extent, viewport)`. `None` if there is no controller or it isn't
    /// bound to a scroll-view yet.
    pub(crate) fn metrics(&self) -> Option<(ElementNodeId, f64, f64, f64)> {
        let ctrl = self.view.controller.as_ref()?;
        let ctrl = ctrl.downcast_ref::<ScrollController>()?;
        let node = ctrl.bound_node?;
        Some((node, ctrl.offset, ctrl.max_scroll_extent, ctrl.viewport_dimension))
    }

    /// Thumb height for the given track length.
    pub(crate) fn thumb_height(track: f64, max_extent: f64, viewport: f64) -> f64 {
        let content = max_extent + viewport;
        if content <= 0.0 {
            return track;
        }
        ((viewport / content) * track).clamp(MIN_THUMB, track)
    }
}

impl Effect for ScrollbarElement {}

impl ElementSubscribe for ScrollbarElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.thickness.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.track_color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.thumb_radius.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for ScrollbarElement {
    fn trace_label(&self) -> String {
        self.metrics()
            .map(|(_, offset, max, vp)| {
                format!("offset={offset:.1} max={max:.1} vp={vp:.1}")
            })
            .unwrap_or_default()
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        self.metrics()
            .map(|(_, offset, max, vp)| {
                vec![
                    ("offset", TraceValue::Num(offset)),
                    ("maxScrollExtent", TraceValue::Num(max)),
                    ("viewportDimension", TraceValue::Num(vp)),
                ]
            })
            .unwrap_or_default()
    }
}

impl ElementOnFocus for ScrollbarElement {}

impl ElementOnGesture for ScrollbarElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        let track = self.cached_track.height;
        if track <= 0.0 {
            return;
        }
        let (node, _offset, max_extent, viewport) = match self.metrics() {
            Some(v) => v,
            None => return,
        };
        // Nothing to scroll (content fits the viewport).
        if max_extent <= 0.0 {
            return;
        }
        let thumb = Self::thumb_height(track, max_extent, viewport);
        let thumb_range = track - thumb;
        if thumb_range <= 0.0 {
            return;
        }

        match event {
            ComposedGestureEvent::PointerDown { local, .. } => {
                cx.request_own_focus();
                let (_, current_offset, _, _) = self.metrics().unwrap();

                // Compute the thumb's current top edge in track-local pixels.
                let thumb_top = (current_offset / max_extent) * thumb_range;
                let on_thumb = local.y >= thumb_top && local.y <= thumb_top + thumb;

                if on_thumb {
                    // Click landed on the thumb: do NOT jump. Drag the thumb
                    // 1:1 with pointer movement from its current position.
                    self.drag = Some(DragState {
                        start_y: local.y,
                        start_offset: current_offset,
                    });
                } else {
                    // Click landed on the track (above or below the thumb):
                    // jump so the thumb's center sits under the pointer, then
                    // drag relative to that post-jump position.
                    let target = ((local.y - thumb / 2.0) / thumb_range * max_extent)
                        .clamp(0.0, max_extent);
                    cx.request_scroll_to(node, target);
                    self.drag = Some(DragState {
                        start_y: local.y,
                        start_offset: target,
                    });
                }
            }
            ComposedGestureEvent::PointerMove { local, .. } => {
                let Some(d) = self.drag else { return; };
                let delta = local.y - d.start_y;
                let new_offset =
                    (d.start_offset + delta * max_extent / thumb_range).clamp(0.0, max_extent);
                cx.request_scroll_to(node, new_offset);
            }
            ComposedGestureEvent::PointerUp { .. } => {
                // Drag ends — clear drag state so the next drag starts fresh.
                self.drag = None;
            }
            ComposedGestureEvent::PointerDoubleDown { .. } => {}
            ComposedGestureEvent::PointerTripleDown { .. } => {}
            ComposedGestureEvent::ContextMenu { .. } => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Factory helpers — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

pub(super) fn prop_val<T: PropValue>(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

pub(super) fn prop_controller(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<JsObject> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    v.as_object().filter(|obj| {
        obj.downcast_ref::<ScrollController>().is_some()
    })
}

pub(super) fn prop_query_key(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Vec<String>> {
    use boa_engine::js_string;
    use boa_engine::object::builtins::JsArray;
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

impl ScrollbarView {
    /// Build a `ScrollbarView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        ScrollbarView {
            controller: prop_controller(props, "controller", ctx),
            color: prop_val::<Brush>(props, "color", ctx),
            track_color: prop_val::<Brush>(props, "trackColor", ctx),
            thumb_radius: prop_val::<f64>(props, "thumbRadius", ctx),
            thickness: prop_val::<f64>(props, "thickness", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
