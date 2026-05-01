pub mod helpers;

use std::any::Any;
use std::rc::Rc;

use crate::core::element::ElementNodeId;
use crate::core::keyboard::Modifiers;

#[derive(Clone)]
pub struct AnyJsCommand(pub(crate) Rc<dyn Any>);

impl AnyJsCommand {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }
}

pub trait IntoAnyJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand;
}

impl IntoAnyJsCommand for AnyJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        self
    }
}

impl IntoAnyJsCommand for PointerInteractJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}

impl IntoAnyJsCommand for FocusableJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}

impl IntoAnyJsCommand for InputJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}

#[derive(Clone)]
pub enum PointerInteractJsCommand {
    Click { x: f64, y: f64 },
}

#[derive(Clone)]
pub enum FocusableJsCommand {
    KeyDown { key: String, code: String, modifiers: Modifiers },
    KeyUp { key: String, code: String, modifiers: Modifiers },
    Focus,
    Blur,
}

#[derive(Clone)]
pub enum InputJsCommand {
    Input { text: String, enter: bool },
    CursorChange { position: usize },
    SelectionChange { anchor: usize, end: usize },
}

impl std::fmt::Debug for AnyJsCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyJsCommand").finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct JsCommandEntry {
    target: ElementNodeId,
    command: AnyJsCommand,
}

#[derive(Debug)]
pub struct JsCommandQueue {
    entries: Vec<JsCommandEntry>,
}

impl Default for JsCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl JsCommandQueue {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, target: ElementNodeId, command: impl IntoAnyJsCommand) {
        self.entries.push(JsCommandEntry { target, command: command.into_any_js_command() });
    }

    pub fn drain(&mut self) -> Vec<(ElementNodeId, AnyJsCommand)> {
        self.entries
            .drain(..)
            .map(|e| (e.target, e.command))
            .collect()
    }
}
