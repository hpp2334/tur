use crate::core::js_event::{AnyJsEvent, FocusableJsEvent};
use crate::core::keyboard::AppKeyEvent;

pub fn make_key_down_event(event: &AppKeyEvent) -> AnyJsEvent {
    AnyJsEvent(std::rc::Rc::new(FocusableJsEvent::KeyDown {
        key: event.key.clone(),
        code: event.code.clone(),
        modifiers: event.modifiers.clone(),
    }))
}

pub fn make_key_up_event(event: &AppKeyEvent) -> AnyJsEvent {
    AnyJsEvent(std::rc::Rc::new(FocusableJsEvent::KeyUp {
        key: event.key.clone(),
        code: event.code.clone(),
        modifiers: event.modifiers.clone(),
    }))
}
