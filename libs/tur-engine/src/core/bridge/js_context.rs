use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_gc::{Finalize, Trace};
use boa_engine::JsData;

use crate::core::elements::ElementTree;
use crate::core::focus::FocusManager;
use crate::core::js_command::JsCommandQueue;

#[derive(Clone, Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurJsContext {
    pub(crate) element_tree: Rc<RefCell<ElementTree>>,
    pub(crate) js_command_queue: Rc<RefCell<JsCommandQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) dirty: Rc<Cell<bool>>,
}

impl TurJsContext {
    pub fn new(
        element_tree: Rc<RefCell<ElementTree>>,
        js_command_queue: Rc<RefCell<JsCommandQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        dirty: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            element_tree,
            js_command_queue,
            focus_manager,
            dirty,
        }
    }
}
