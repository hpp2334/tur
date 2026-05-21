use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::event::{AppEvent, AppGestureEvent};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;
use crate::core::js_command::PointerInteractJsCommand;
use crate::core::pointer_region::PointerRegionTracker;
use crate::elements::PointerInteractElement;

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
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Gesture(AppGestureEvent::PointerMove { position }) = event else {
            return;
        };

        let hit_path = HitTest::new(&*cx.element_tree).path(*position);
        let filtered = filter_opaque_path(&hit_path, &*cx.element_tree);
        let diff = self.tracker.update(&filtered, |id| {
            has_pointer_region_callbacks(&*cx.element_tree, id)
        });

        for id in &diff.entered {
            cx.js_command_queue
                .push(*id, PointerInteractJsCommand::PointerEnter);
        }

        for id in &diff.exited {
            cx.js_command_queue
                .push(*id, PointerInteractJsCommand::PointerExit);
        }
    }
}

fn has_pointer_region_callbacks(tree: &ElementTree, id: ElementNodeId) -> bool {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .map(|e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.has_pointer_region_callbacks())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn is_pointer_region_opaque(tree: &ElementTree, id: ElementNodeId) -> bool {
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .map(|e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.is_pointer_region_opaque())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn filter_opaque_path(path: &[ElementNodeId], tree: &ElementTree) -> Vec<ElementNodeId> {
    let mut result = Vec::new();
    for &id in path {
        result.push(id);
        if is_pointer_region_opaque(tree, id) {
            break;
        }
    }
    result
}
