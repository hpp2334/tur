use std::cell::RefCell;
use std::rc::Rc;

use crate::core::layout::{Axis, Size};
use crate::core::render::brush::Brush;

use crate::builtin_plugins::scroll::ScrollEvent;
use crate::builtin_plugins::scroll::core::controller::ScrollController;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{
    AnyElement, ElementOnWheel, ElementOnWheelContext, ElementTrace, TraceValue, WheelEvent,
};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx, read_val};

use super::scroll_position::ScrollPosition;

// ---------------------------------------------------------------------------
// ScrollViewView — the user's declaration. Pure Rust.
//
// `axis`, `padding`, and `color` are reactive (`Val<T>`).
// `controller` is a shared `ScrollController` — bound eagerly at build time
// (not reactive).  `child` is required (the scrollable content).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ScrollViewView {
    pub(crate) axis: Option<Val<Axis>>,
    pub(crate) padding: Option<Val<f64>>,
    pub(crate) color: Option<Val<Brush>>,
    /// Shared `ScrollController` — bound to this node at build time (not
    /// reactive).
    pub(crate) controller: Option<Rc<RefCell<ScrollController>>>,
    /// Rut rail: a static initial pixel offset applied once after the
    /// first content layout (the `ScrollController::pending_initial_offset`
    /// twin).
    pub(crate) initial_offset: Option<Val<f64>>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) child: Rc<dyn View>,
}

impl View for ScrollViewView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        // Resolve axis eagerly — the wheel handler and controller-metric
        // updates need it at event time where no store/Context is available.
        let axis = self
            .axis
            .as_ref()
            .and_then(|v| read_val(cx, v))
            .unwrap_or(Axis::Vertical);

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        let pending_view_initial = self
            .initial_offset
            .as_ref()
            .and_then(|v| read_val(cx, v));
        cx.insert_node(
            id,
            AnyElement::with_wheel(ScrollViewElement {
                view: self.clone(),
                axis,
                position: ScrollPosition::new(),
                painting: ScrollViewPainting::default(),
                pending_view_initial,
            })
            .with_callbacks(),
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        // Bind the controller to this node so `jump_to` (and drag-driven
        // `ScrollTo` events from a sibling Scrollbar) can locate this element.
        if let Some(ctrl) = &self.controller {
            let mut ctrl = ctrl.borrow_mut();
            ctrl.bound_node = Some(id);
            ctrl.element_tree = Some(cx.node_tree());
            ctrl.mutation_queue = Some(cx.mutation_queue());
            ctrl.dirty_flag = Some(cx.dirty());
        }
        let _child_id = self.child.build(cx, id.into());
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
    /// The rut rail's one-shot initial offset (resolved from the view's
    /// `initial_offset` val at build; consumed by the first layout).
    pub(crate) pending_view_initial: Option<f64>,
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
        let Some(ref ctrl) = self.view.controller else {
            return;
        };
        let mut ctrl = ctrl.borrow_mut();
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
        if let Some(ref ctrl) = self.view.controller {
            let mut ctrl = ctrl.borrow_mut();
            let Some(initial) = ctrl.pending_initial_offset.take() else {
                return;
            };
            let clamped = initial.clamp(0.0, self.position.max_scroll_extent());
            self.position.correct_pixels(clamped);
            ctrl.offset = clamped;
        } else if let Some(initial) = self.pending_view_initial.take() {
            let clamped = initial.clamp(0.0, self.position.max_scroll_extent());
            self.position.correct_pixels(clamped);
        }
    }

    /// Fire the controller's `onScroll` mutation for a layout-driven pixels
    /// correction — the content-shrink clamp in `perform_layout` (Flutter
    /// fires scroll notifications when `applyContentDimensions` moves
    /// pixels). The event is pushed onto the mutation queue, so the flush
    /// loop invokes it after layout in the same frame — the same rail the
    /// wheel path uses. Metrics are read from the controller, which
    /// `update_controller_metrics` has already synced against the clamped
    /// position.
    pub(super) fn notify_layout_driven_scroll(
        &self,
        cx: &crate::core::layout::LayoutContext<'_>,
    ) {
        let Some(ref ctrl) = self.view.controller else {
            return;
        };
        let ctrl = ctrl.borrow();
        let Some(on_scroll) = ctrl.on_scroll else {
            return;
        };
        cx.mutation_queue.borrow_mut().push(
            on_scroll,
            ScrollEvent::new(ctrl.offset, ctrl.max_scroll_extent, ctrl.viewport_dimension),
        );
    }
}

impl Lifecycle for ScrollViewElement {}

impl ElementSubscribe for ScrollViewElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.padding.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.color.as_ref() {
            cx.subscribe_val(v);
        }
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
            (
                "maxScrollExtent",
                TraceValue::Num(self.position.max_scroll_extent()),
            ),
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
            if let Some(ref ctrl) = self.view.controller {
                let ctrl = ctrl.borrow();
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
            cx.request_paint();
        }

        overscroll
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl ScrollViewView {
    /// Rut-rail constructor (`core::rut_runtime`): axis + child, no
    /// controller (wheel scrolling works without one).
    pub(crate) fn new_rut(axis: Option<Val<Axis>>, child: Rc<dyn View>) -> Self {
        ScrollViewView {
            axis,
            padding: None,
            color: None,
            controller: None,
            initial_offset: None,
            query_key: None,
            child,
        }
    }
}
