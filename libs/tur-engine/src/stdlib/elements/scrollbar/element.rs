use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{Brush, Size};

use crate::core::bridge::JsProps;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{
    AnyElement, ComposedGestureEvent, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementTrace, TraceValue,
};
use crate::stdlib::scroll::ScrollController;
use crate::core::view::{ViewCx, Lifecycle, Val, View};

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
    pub(crate) controller: Option<JsObject>,
    pub(crate) color: Option<Val<Brush>>,
    pub(crate) track_color: Option<Val<Brush>>,
    pub(crate) thumb_radius: Option<Val<f64>>,
    /// Track thickness (width for a vertical scrollbar). Defaults to 10.
    pub(crate) thickness: Option<Val<f64>>,
    pub(crate) query_key: Option<Vec<String>>,
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
    pub(crate) track_color: Option<Brush>,
    pub(crate) color: Option<Brush>,
    pub(crate) thumb_radius: Option<f64>,
}

pub struct ScrollbarElement {
    pub(crate) view: ScrollbarView,
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

impl Lifecycle for ScrollbarElement {}

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
    ) -> bool {
        let track = self.cached_track.height;
        if track <= 0.0 {
            return true;
        }
        let (node, _offset, max_extent, viewport) = match self.metrics() {
            Some(v) => v,
            None => return true,
        };
        if max_extent <= 0.0 {
            return true;
        }
        let thumb = Self::thumb_height(track, max_extent, viewport);
        let thumb_range = track - thumb;
        if thumb_range <= 0.0 {
            return true;
        }

        match event {
            ComposedGestureEvent::PointerDown { local, .. } => {
                cx.request_own_focus();
                let (_, current_offset, _, _) = self.metrics().unwrap();

                let thumb_top = (current_offset / max_extent) * thumb_range;
                let on_thumb = local.y >= thumb_top && local.y <= thumb_top + thumb;

                if on_thumb {
                    self.drag = Some(DragState {
                        start_y: local.y,
                        start_offset: current_offset,
                    });
                } else {
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
                let Some(d) = self.drag else { return true; };
                let delta = local.y - d.start_y;
                let new_offset =
                    (d.start_offset + delta * max_extent / thumb_range).clamp(0.0, max_extent);
                cx.request_scroll_to(node, new_offset);
            }
            ComposedGestureEvent::PointerUp { .. } => {
                self.drag = None;
            }
            ComposedGestureEvent::PointerDoubleDown { .. } => {}
            ComposedGestureEvent::PointerTripleDown { .. } => {}
            ComposedGestureEvent::Click { .. } => {}
            ComposedGestureEvent::ContextMenu { .. } => {}
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Factory helpers — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl ScrollbarView {
    /// Build a `ScrollbarView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        let controller = p
            .raw_opt("controller")
            .and_then(|v| v.as_object())
            .filter(|obj| obj.downcast_ref::<ScrollController>().is_some());
        ScrollbarView {
            controller,
            color: p.val::<Brush>("color"),
            track_color: p.val::<Brush>("trackColor"),
            thumb_radius: p.val::<f64>("thumbRadius"),
            thickness: p.val::<f64>("thickness"),
            query_key: p.query_key("queryKey"),
        }
    }
}
