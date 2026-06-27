use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::event::{AppEvent, AppGestureEvent};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;
use crate::core::pointer_region::PointerRegionTracker;
use crate::elements::mouse_region::{MouseRegionElement, PointerRegionEvent};
use tur_shared::{Cursor, Offset};

pub struct PointerRegionAppHandler {
    tracker: PointerRegionTracker,
    /// Last cursor emitted via `set_cursor`. Used to avoid re-emitting the
    /// same value on every pointer move.
    last_cursor: Option<Cursor>,
}

impl Default for PointerRegionAppHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerRegionAppHandler {
    pub fn new() -> Self {
        Self {
            tracker: PointerRegionTracker::new(),
            last_cursor: None,
        }
    }
}

impl AppHandler for PointerRegionAppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Gesture(AppGestureEvent::PointerMove { position }) = event else {
            return;
        };
        let position = *position;

        let hit_path = HitTest::new(&*cx.element_tree).path(position);
        let filtered = filter_opaque_path(&hit_path, &*cx.element_tree);
        let diff = self.tracker.update(&filtered, |id| {
            has_region_callbacks(&*cx.element_tree, id)
        });

        for id in &diff.exited {
            let Some(m) = mouse_region_exit_mutation(&*cx.element_tree, *id) else {
                continue;
            };
            let local = local_position(&*cx.element_tree, *id, position);
            cx.mutation_queue
                .push(m, PointerRegionEvent { local, global: position });
        }

        for id in &diff.entered {
            let Some(m) = mouse_region_enter_mutation(&*cx.element_tree, *id) else {
                continue;
            };
            let local = local_position(&*cx.element_tree, *id, position);
            cx.mutation_queue
                .push(m, PointerRegionEvent { local, global: position });
        }

        // Resolve the active cursor: the inner-most (deepest) MouseRegion
        // with a cursor wins. `filtered` is ordered [deepest, ..., outermost],
        // so iterate normally and return the first match.
        let mut active_cursor: Option<Cursor> = None;
        for id in &filtered {
            if let Some(cursor) = cursor_for(&*cx.element_tree, *id) {
                active_cursor = Some(cursor);
                break;
            }
        }
        let new_cursor = active_cursor.unwrap_or(Cursor::Default);
        if self.last_cursor != Some(new_cursor) {
            cx.set_cursor(new_cursor);
            self.last_cursor = Some(new_cursor);
        }
    }
}

fn mouse_region_enter_mutation(
    tree: &ElementTree,
    id: ElementNodeId,
) -> Option<crate::core::edgy_event::EdgyMutation<PointerRegionEvent>> {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .and_then(|m| m.component.on_enter)
}

fn mouse_region_exit_mutation(
    tree: &ElementTree,
    id: ElementNodeId,
) -> Option<crate::core::edgy_event::EdgyMutation<PointerRegionEvent>> {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .and_then(|m| m.component.on_exit)
}

fn has_region_callbacks(tree: &ElementTree, id: ElementNodeId) -> bool {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .map(|m| m.has_region_callbacks())
        .unwrap_or(false)
}

fn is_region_opaque(tree: &ElementTree, id: ElementNodeId) -> bool {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .map(|m| m.is_region_opaque())
        .unwrap_or(false)
}

/// Resolve the cursor for a MouseRegion element from its layout-resolved
/// `cursor` field. Returns `None` for elements without a cursor. (The value is
/// materialized during layout; the handler runs without a store/Context.)
fn cursor_for(tree: &ElementTree, id: ElementNodeId) -> Option<Cursor> {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())?
        .resolved_cursor()
}

fn filter_opaque_path(path: &[ElementNodeId], tree: &ElementTree) -> Vec<ElementNodeId> {
    let mut result = Vec::new();
    for &id in path {
        result.push(id);
        if is_region_opaque(tree, id) {
            break;
        }
    }
    result
}

/// Compute a position relative to the element's top-left by walking parents
/// and subtracting each one's layout offset. Mirrors the helper in
/// `handlers/gesture.rs`; kept local to avoid coupling.
fn local_position(tree: &ElementTree, node_id: ElementNodeId, global: Offset) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current = Some(node_id);
    while let Some(cid) = current {
        if let Some(n) = tree.get(cid) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else {
            break;
        }
    }
    Offset::new(global.x - abs_x, global.y - abs_y)
}
