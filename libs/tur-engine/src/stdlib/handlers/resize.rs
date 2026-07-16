use crate::core::event::PlatformEvent;
use crate::core::handler::{AppHandler, HandlerContext};

pub struct ResizeHandler;

impl AppHandler for ResizeHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        let PlatformEvent::Resize { logical_width, logical_height, dpr } = event else {
            return;
        };

        cx.renderer.resize(*logical_width, *logical_height, *dpr);
        *cx.size = (*logical_width as f64, *logical_height as f64);
        cx.element_tree.mark_root_dirty();
        cx.request_draw();
    }
}
