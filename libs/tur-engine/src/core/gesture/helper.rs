use crate::core::js_event::{AnyJsEvent, PointerInteractJsEvent};

pub fn make_click_event(x: f64, y: f64) -> AnyJsEvent {
    AnyJsEvent(std::rc::Rc::new(PointerInteractJsEvent::Click { x, y }))
}
