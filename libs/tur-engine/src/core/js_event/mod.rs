pub mod helpers;

use std::any::Any;
use std::rc::Rc;

use crate::core::element::ElementNodeId;
use crate::core::keyboard::Modifiers;

#[derive(Clone)]
pub struct AnyJsEvent(pub(crate) Rc<dyn Any>);

impl AnyJsEvent {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }
}

pub trait IntoAnyJsEvent {
    fn into_any_js_event(self) -> AnyJsEvent;
}

impl IntoAnyJsEvent for AnyJsEvent {
    fn into_any_js_event(self) -> AnyJsEvent {
        self
    }
}

impl IntoAnyJsEvent for PointerInteractJsEvent {
    fn into_any_js_event(self) -> AnyJsEvent {
        AnyJsEvent(Rc::new(self))
    }
}

impl IntoAnyJsEvent for FocusableJsEvent {
    fn into_any_js_event(self) -> AnyJsEvent {
        AnyJsEvent(Rc::new(self))
    }
}

impl IntoAnyJsEvent for InputJsEvent {
    fn into_any_js_event(self) -> AnyJsEvent {
        AnyJsEvent(Rc::new(self))
    }
}

#[derive(Clone)]
pub enum PointerInteractJsEvent {
    Click { x: f64, y: f64 },
}

#[derive(Clone)]
pub enum FocusableJsEvent {
    KeyDown { key: String, code: String, modifiers: Modifiers },
    KeyUp { key: String, code: String, modifiers: Modifiers },
    Focus,
    Blur,
}

#[derive(Clone)]
pub enum InputJsEvent {
    Input { text: String, enter: bool },
    CursorChange { position: usize },
    SelectionChange { anchor: usize, end: usize },
}

struct JsEventEntry {
    target: ElementNodeId,
    event: AnyJsEvent,
}

pub struct JsEventQueue {
    entries: Vec<JsEventEntry>,
}

impl Default for JsEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl JsEventQueue {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, target: ElementNodeId, event: impl IntoAnyJsEvent) {
        self.entries.push(JsEventEntry { target, event: event.into_any_js_event() });
    }

    pub fn drain(&mut self) -> Vec<(ElementNodeId, AnyJsEvent)> {
        self.entries
            .drain(..)
            .map(|e| (e.target, e.event))
            .collect()
    }
}
