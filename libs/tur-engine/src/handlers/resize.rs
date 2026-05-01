use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};

pub struct ResizeHandler;

impl AppHandler for ResizeHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Resize { logical_width, logical_height, dpr } = event else {
            return;
        };

        cx.renderer.resize(*logical_width, *logical_height, *dpr);
        *cx.size = (*logical_width as f64, *logical_height as f64);
        cx.request_draw();
    }
}
