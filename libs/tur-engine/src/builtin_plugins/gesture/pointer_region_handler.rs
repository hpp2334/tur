use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::NodeTreeData;
use crate::core::platform::{PlatformEvent, PointerDeviceKind, PointerInput};
use crate::core::hit_test::HitTest;
use crate::core::layout::Offset;
use crate::core::edgy::mutation::MutationHandle;
use crate::builtin_plugins::gesture::pointer_region_tracker::PointerRegionTracker;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::builtin_plugins::gesture::mouse_region::{MouseRegionElement, PointerRegionEvent};

/// Tracks `onEnter` / `onExit` callbacks for `MouseRegion`s as the pointer
/// moves. Cursor resolution lives in the paint pass (see `MouseRegion::paint`
/// and `PaintContext::set_cursor`); this subsystem only fires enter/exit
/// mutations.
pub struct PointerSubsystem {
    tracker: PointerRegionTracker,
}

impl Default for PointerSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerSubsystem {
    pub fn new() -> Self {
        Self {
            tracker: PointerRegionTracker::new(),
        }
    }
}

impl Subsystem for PointerSubsystem {
    fn handle_platform_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        let PlatformEvent::Pointer(PointerInput::PointerMove {
            position,
            device: PointerDeviceKind::Mouse,
            time_ms: _,
        }) = event
        else {
            return;
        };
        let position = *position;

        let (exited, entered) = {
            let tree = cx.element_tree.borrow();
            let hit_path = HitTest::new(&tree).path(position);
            let filtered = filter_opaque_path(&hit_path, &tree);
            let diff = self
                .tracker
                .update(&filtered, |id| has_region_callbacks(&tree, id));
            let exited: Vec<ElementNodeId> = diff.exited.to_vec();
            let entered: Vec<ElementNodeId> = diff.entered.to_vec();
            (exited, entered)
        };

        let mut mq = cx.mutation_queue.borrow_mut();
        let tree = cx.element_tree.borrow();
        for id in &exited {
            let Some(m) = mouse_region_exit_mutation(&tree, *id) else {
                continue;
            };
            let local = local_position(&tree, *id, position);
            mq.push(m, PointerRegionEvent { local, global: position });
        }

        for id in &entered {
            let Some(m) = mouse_region_enter_mutation(&tree, *id) else {
                continue;
            };
            let local = local_position(&tree, *id, position);
            mq.push(m, PointerRegionEvent { local, global: position });
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
