use crate::core::mutation::MutationHandle;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::NodeTreeData;
use crate::core::event::{PlatformEvent, PointerDeviceKind, PointerInput};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;
use crate::elements::mouse_region::{MouseRegionElement, PointerRegionEvent};
use tur_shared::Offset;

use crate::stdlib::pointer_region::PointerRegionTracker;

/// Tracks `onEnter` / `onExit` callbacks for `MouseRegion`s as the pointer
/// moves. Cursor resolution lives in the paint pass (see `MouseRegion::paint`
/// and `PaintContext::set_cursor`); this handler only fires enter/exit
/// mutations.
pub struct PointerRegionAppHandler {
    tracker: PointerRegionTracker,
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
        }
    }
}

impl AppHandler for PointerRegionAppHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        let PlatformEvent::Pointer(PointerInput::PointerMove {
            position,
            device: PointerDeviceKind::Mouse,
        }) = event
        else {
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
    }
}

fn mouse_region_enter_mutation(
    tree: &NodeTreeData,
    id: ElementNodeId,
) -> Option<MutationHandle<PointerRegionEvent>> {
    tree.get_element(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .and_then(|m| m.view.on_enter)
}

fn mouse_region_exit_mutation(
    tree: &NodeTreeData,
    id: ElementNodeId,
) -> Option<MutationHandle<PointerRegionEvent>> {
    tree.get_element(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .and_then(|m| m.view.on_exit)
}

fn has_region_callbacks(tree: &NodeTreeData, id: ElementNodeId) -> bool {
    tree.get_element(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .map(|m| m.has_region_callbacks())
        .unwrap_or(false)
}

fn is_region_opaque(tree: &NodeTreeData, id: ElementNodeId) -> bool {
    tree.get_element(id)
        .and_then(|node| node.element.as_ref())
        .and_then(|e| e.cast::<MouseRegionElement>())
        .map(|m| m.is_region_opaque())
        .unwrap_or(false)
}

fn filter_opaque_path(path: &[ElementNodeId], tree: &NodeTreeData) -> Vec<ElementNodeId> {
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
/// and subtracting each one's layout offset. Hops through fragment ancestors
/// transparently (fragments have zero offset, so they're skipped without
/// affecting the sum). Mirrors the helper in `handlers/gesture/mod.rs`; kept
/// local to avoid coupling.
fn local_position(tree: &NodeTreeData, node_id: ElementNodeId, global: Offset) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current: Option<NodeId> = Some(node_id.into());
    while let Some(cid) = current {
        if let Some(n) = tree.get_element(ElementNodeId::new(cid.as_u64())) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else if let Some(f) = tree.get_fragment(FragmentNodeId::new(cid.as_u64())) {
            current = Some(f.parent);
        } else {
            break;
        }
    }
    Offset::new(global.x - abs_x, global.y - abs_y)
}
