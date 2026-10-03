use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::builtin_plugins::lazy_container::lazy_grid::{LazyGridElement, VisibleRangeChangeEvent};
use crate::builtin_plugins::scroll::ScrollEvent;
use crate::core::edgy::mutation::{MutationHandle, PendingMutationInvocationQueue};
use crate::core::element::ElementNodeId;

/// Plain-Rust grid controller — the programmatic handle over a bound
/// `LazyGrid`. Whoever binds it sets the bound node + the tree/mutation
/// handles; `jump_to` drives the bound element and fires the scroll /
/// visible-range callbacks through the mutation queue.
pub struct LazyGridController {
    pub(crate) offset: f64,
    pub(crate) max_scroll_extent: f64,
    pub(crate) viewport_dimension: f64,
    pub(crate) on_scroll: Option<MutationHandle<ScrollEvent>>,
    pub(crate) on_visible_range_change: Option<MutationHandle<VisibleRangeChangeEvent>>,
    /// The lazy-grid node this controller is bound to.
    pub(crate) bound_node: Option<ElementNodeId>,
    pub(crate) element_tree: Option<crate::core::elements::NodeTree>,
    pub(crate) mutation_queue: Option<Rc<RefCell<PendingMutationInvocationQueue>>>,
    pub(crate) dirty_flag: Option<Rc<Cell<bool>>>,
}

impl LazyGridController {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            max_scroll_extent: 0.0,
            viewport_dimension: 0.0,
            on_scroll: None,
            on_visible_range_change: None,
            bound_node: None,
            element_tree: None,
            mutation_queue: None,
            dirty_flag: None,
        }
    }

    /// Programmatically scroll the bound lazy grid to `target_offset`
    /// (clamped to the content). Corrects the element's position, syncs the
    /// controller metrics, marks the tree dirty, and fires
    /// `onVisibleRangeChange` + `onScroll` via the mutation queue. A no-op
    /// when the controller isn't bound yet.
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
        let on_visible_range_change = self.on_visible_range_change;

        let mut tree = element_tree_rc.borrow_mut();
        let Some(node) = tree.get_element_mut(node_id) else {
            return;
        };
        let Some(ref mut element) = node.element else {
            return;
        };
        let Some(lg) = element.cast_mut::<LazyGridElement>() else {
            return;
        };

        let max = lg.position.max_scroll_extent();
        let clamped = target_offset.clamp(0.0, max);
        lg.position.correct_pixels(clamped);

        let vp = lg.position.viewport_size();
        let dim = match lg.axis {
            crate::core::layout::Axis::Vertical => vp.height,
            crate::core::layout::Axis::Horizontal => vp.width,
        };

        let viewport_main = crate::core::layout::Axis::main(&lg.axis, vp);
        let (start, end) = lg.compute_visible_range(viewport_main);

        let new_offset = lg.position.pixels();
        tree.mark_dirty(node_id.into());
        drop(tree);

        self.offset = new_offset;
        self.max_scroll_extent = max;
        self.viewport_dimension = dim;
        dirty_flag.set(true);

        if let Some(queue_rc) = self.mutation_queue.as_ref() {
            if let Some(m) = on_visible_range_change {
                queue_rc.borrow_mut().push(
                    m,
                    VisibleRangeChangeEvent {
                        start_index: start,
                        end_index: end,
                    },
                );
            }
            if let Some(m) = on_scroll {
                queue_rc.borrow_mut().push(
                    m,
                    ScrollEvent::new(
                        self.offset,
                        self.max_scroll_extent,
                        self.viewport_dimension,
                    ),
                );
            }
        }
    }
}

impl Default for LazyGridController {
    fn default() -> Self {
        Self::new()
    }
}
