use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::edgy::mutation::{MutationHandle, PendingMutationInvocationQueue};
use crate::core::element::ElementNodeId;

use super::ScrollEvent;
use crate::builtin_plugins::scroll::scroll_view::ScrollViewElement;

/// Plain-Rust scroll controller — the programmatic handle over a bound
/// `ScrollView`. Views bind it at build time (`ScrollViewView::build` sets
/// the bound node + the tree/mutation handles); `jump_to` drives the bound
/// scroll element and fires `on_scroll` through the mutation queue — the
/// same rail the wheel path uses.
pub struct ScrollController {
    pub(crate) offset: f64,
    pub(crate) max_scroll_extent: f64,
    pub(crate) viewport_dimension: f64,
    pub(crate) on_scroll: Option<MutationHandle<ScrollEvent>>,
    /// The scroll-view node this controller is bound to. Set at build time
    /// by `ScrollViewView::build`. `jump_to`/drag use this to locate the
    /// scroll element.
    pub(crate) bound_node: Option<ElementNodeId>,
    pub(crate) element_tree: Option<crate::core::elements::NodeTree>,
    pub(crate) mutation_queue: Option<Rc<RefCell<PendingMutationInvocationQueue>>>,
    pub(crate) dirty_flag: Option<Rc<Cell<bool>>>,
    /// One-shot initial offset (the `initialOffset` twin): consumed by the
    /// element's first layout via `apply_pending_initial_offset`.
    pub(crate) pending_initial_offset: Option<f64>,
}

impl ScrollController {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            max_scroll_extent: 0.0,
            viewport_dimension: 0.0,
            on_scroll: None,
            bound_node: None,
            element_tree: None,
            mutation_queue: None,
            dirty_flag: None,
            pending_initial_offset: None,
        }
    }

    /// Programmatically scroll the bound scroll view to `target_offset`
    /// (clamped to the content). Corrects the element's position, syncs the
    /// controller metrics, marks the tree dirty, and fires `on_scroll` via
    /// the mutation queue. A no-op when the controller isn't bound yet.
    pub fn jump_to(&mut self, target_offset: f64) {
        let Some(element_tree_rc) = self.element_tree.clone() else {
            return;
        };
        let Some(dirty_flag) = self.dirty_flag.clone() else {
            return;
        };
        let Some(node_id) = self.bound_node else {
            return;
        };
        let on_scroll = self.on_scroll;

        let mut tree = element_tree_rc.borrow_mut();
        let Some(node) = tree.get_element_mut(node_id) else {
            return;
        };
        let Some(ref mut element) = node.element else {
            return;
        };
        let Some(sv) = element.cast_mut::<ScrollViewElement>() else {
            return;
        };

        let max = sv.position.max_scroll_extent();
        let clamped = target_offset.clamp(0.0, max);
        sv.position.correct_pixels(clamped);

        let vp = sv.viewport_size();
        let dim = match sv.axis() {
            crate::core::layout::Axis::Vertical => vp.height,
            crate::core::layout::Axis::Horizontal => vp.width,
        };
        let new_offset = sv.position.pixels();
        tree.mark_dirty(node_id.into());
        drop(tree);

        self.offset = new_offset;
        self.max_scroll_extent = max;
        self.viewport_dimension = dim;
        dirty_flag.set(true);

        if let Some(queue_rc) = self.mutation_queue.as_ref()
            && let Some(m) = on_scroll
        {
            queue_rc.borrow_mut().push(
                m,
                ScrollEvent {
                    offset: self.offset,
                    max_extent: self.max_scroll_extent,
                    viewport_dimension: self.viewport_dimension,
                },
            );
        }
    }
}

impl Default for ScrollController {
    fn default() -> Self {
        Self::new()
    }
}
