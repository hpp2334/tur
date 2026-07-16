use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

/// Snapshot of editable-text state used by `UndoController` for undo/redo.
/// Captures plain text plus cursor/selection byte offsets — not spans,
/// because spans are re-derived from text by the JS layer's `onInput`
/// callback (e.g. via syntax-highlight re-tokenization) after a restore.
#[derive(Clone, Debug, Default)]
pub struct TextEditingValue {
    pub(crate) text: String,
    pub(crate) cursor_position: usize,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
}

impl TextEditingValue {
    pub fn from_controller(c: &crate::stdlib::text::controller::TextEditingController) -> Self {
        TextEditingValue {
            text: c.text(),
            cursor_position: c.cursor_position(),
            selection_anchor: c.selection_anchor(),
            selection_end: c.selection_end(),
        }
    }
}

/// Flutter-style undo/redo history stack. Pairs with a
/// `TextEditingController` (passed to `InputEdgy` via the `undoController`
/// prop). The controller owns the *current* value; this object owns the
/// *history*. Each call to `push` records a prior state (cleared on push,
/// matching the standard "redo branch is abandoned when the user types
/// again" convention).
#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct UndoController {
    undo_stack: Vec<TextEditingValue>,
    redo_stack: Vec<TextEditingValue>,
    /// Max entries per stack. Older entries are dropped FIFO.
    limit: usize,
}

impl Default for UndoController {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoController {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            limit: 100,
        }
    }

    /// Record a prior state (the value BEFORE a text-mutating keystroke).
    /// Always clears the redo stack — branching backward is abandoned
    /// once the user types something new.
    pub fn push(&mut self, prior: TextEditingValue) {
        if self.undo_stack.len() >= self.limit {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(prior);
        self.redo_stack.clear();
    }

    /// Pop the previous state from the undo stack and return it so the
    /// caller can apply it to the controller. `current` (the controller's
    /// present state) is pushed onto the redo stack so the change can be
    /// re-applied. Returns `None` if the undo stack is empty.
    pub fn undo(&mut self, current: TextEditingValue) -> Option<TextEditingValue> {
        let prior = self.undo_stack.pop()?;
        self.redo_stack.push(current);
        Some(prior)
    }

    /// Pop the next state from the redo stack. `current` is pushed onto the
    /// undo stack so the change can be undone again.
    pub fn redo(&mut self, current: TextEditingValue) -> Option<TextEditingValue> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Class for UndoController {
    const NAME: &'static str = "UndoController";
    const LENGTH: usize = 0;

    fn data_constructor(
        _new_target: &JsValue,
        _args: &[JsValue],
        _ctx: &mut Context,
    ) -> JsResult<Self> {
        Ok(Self::new())
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        // canUndo / canRedo getters — cheap booleans for menu-item enabling.
        let can_undo_getter = NativeFunction::from_fn_ptr(|this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<UndoController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            Ok(JsValue::from(ctrl.can_undo()))
        })
        .to_js_function(class.context().realm());
        class.accessor(
            js_string!("canUndo"),
            Some(can_undo_getter),
            None,
            Attribute::default(),
        );

        let can_redo_getter = NativeFunction::from_fn_ptr(|this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<UndoController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            Ok(JsValue::from(ctrl.can_redo()))
        })
        .to_js_function(class.context().realm());
        class.accessor(
            js_string!("canRedo"),
            Some(can_redo_getter),
            None,
            Attribute::default(),
        );

        // clear() — reset both stacks.
        class.method(
            js_string!("clear"),
            0,
            NativeFunction::from_fn_ptr(|this, _, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<UndoController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                ctrl.clear();
                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}
