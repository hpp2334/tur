use std::any::Any;
use std::rc::Rc;

use crate::core::element::ElementNodeId;
use crate::core::keyboard::Modifiers;

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
    event: Rc<dyn Any>,
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

    pub fn push(&mut self, target: ElementNodeId, event: Rc<dyn Any>) {
        self.entries.push(JsEventEntry { target, event });
    }

    pub fn drain(&mut self) -> Vec<(ElementNodeId, Rc<dyn Any>)> {
        self.entries
            .drain(..)
            .map(|e| (e.target, e.event))
            .collect()
    }
}
