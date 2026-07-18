use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};

use crate::core::handlers::wheel::dispatch_wheel;

pub struct ScrollChainingHandler;

impl AppHandler for ScrollChainingHandler {
    fn handle_app_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ScrollOverscroll { source_id, delta } = event else {
            return;
        };
        let (source_id, delta) = (*source_id, *delta);

        let Some(parent_id) = find_ancestor_with_wheel(&*cx.element_tree, source_id) else {
            return;
        };

        let overscroll = dispatch_wheel(cx, parent_id, 0.0, delta);
        if overscroll.abs() > 0.001 {
            cx.app_event_queue.push(AppEvent::ScrollOverscroll {
                source_id: parent_id,
                delta: overscroll,
            });
        }
    }
}

/// Walk parents from `start` to find the nearest ancestor with an `onWheel`
/// handler. Hops through fragment ancestors transparently (fragments can't
/// carry wheel handlers, so they're skipped without inspection).
fn find_ancestor_with_wheel(
    tree: &crate::core::elements::NodeTreeData,
    start: ElementNodeId,
) -> Option<ElementNodeId> {
    let mut current: Option<NodeId> = tree.get_element(start).and_then(|n| n.parent);
    while let Some(id) = current {
        if let Some(node) = tree.get_element(ElementNodeId::new(id.as_u64())) {
            if let Some(ref element) = node.element {
                if element.has_on_wheel() {
                    return Some(ElementNodeId::new(id.as_u64()));
                }
            }
            current = node.parent;
        } else if let Some(frag) = tree.get_fragment(FragmentNodeId::new(id.as_u64())) {
            current = Some(frag.parent);
        } else {
            break;
        }
    }
    None
}
