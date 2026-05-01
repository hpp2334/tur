use std::cell::Cell;

use crate::core::elements::ElementTree;
use crate::core::event::queue::AppEventQueue;
use crate::core::focus::FocusManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::js_command::JsCommandQueue;
use crate::core::render::Renderer;

pub trait AppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &crate::core::event::AppEvent);
}

pub struct HandlerContext<'a> {
    pub element_tree: &'a mut ElementTree,
    pub focus_manager: &'a mut FocusManager,
    pub js_command_queue: &'a mut JsCommandQueue,
    pub event_queue: &'a mut AppEventQueue,
    pub gesture_composer: &'a mut GestureEventComposer,
    pub renderer: &'a mut dyn Renderer,
    pub size: &'a mut (f64, f64),
    pub needs_draw: &'a Cell<bool>,
}

impl<'a> HandlerContext<'a> {
    pub fn request_draw(&self) {
        self.needs_draw.set(true);
    }
}
