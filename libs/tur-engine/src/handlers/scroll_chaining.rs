use crate::core::element::ElementNodeId;
use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::handlers::wheel::dispatch_wheel;

pub struct ScrollChainingHandler;

impl AppHandler for ScrollChainingHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ScrollOverscroll { source_id, delta } = event else {
            return;
        };
        let (source_id, delta) = (*source_id, *delta);

        let Some(parent_id) = find_ancestor_with_wheel(&*cx.element_tree, source_id) else {
            return;
        };

        let overscroll = dispatch_wheel(cx, parent_id, 0.0, delta);
        if overscroll.abs() > 0.001 {
            cx.event_queue.push(AppEvent::ScrollOverscroll {
                source_id: parent_id,
                delta: overscroll,
            });
        }
    }
}

fn find_ancestor_with_wheel(
    tree: &crate::core::elements::ElementTree,
    start: ElementNodeId,
) -> Option<ElementNodeId> {
    let mut current = tree.get(start).and_then(|n| n.parent);
    while let Some(id) = current {
        if let Some(node) = tree.get(id) {
            if let Some(ref element) = node.element {
                if element.has_on_wheel() {
                    return Some(id);
                }
            }
            current = node.parent;
        } else {
            break;
        }
    }
    None
}
