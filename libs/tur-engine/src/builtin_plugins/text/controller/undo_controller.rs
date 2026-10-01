/// Snapshot of editable-text state used by `UndoController` for undo/redo.
/// Captures plain text plus cursor/selection byte offsets — not spans,
/// because spans are re-derived from text by the input callback (e.g. via
/// syntax-highlight re-tokenization) after a restore.
#[derive(Clone, Debug, Default)]
pub struct TextEditingValue {
    pub(crate) text: String,
    pub(crate) cursor_position: usize,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
}

impl TextEditingValue {
    pub fn from_controller(
        c: &crate::builtin_plugins::text::controller::TextEditingController,
    ) -> Self {
        TextEditingValue {
            text: c.text(),
            cursor_position: c.cursor_position(),
            selection_anchor: c.selection_anchor(),
            selection_end: c.selection_end(),
        }
    }
}

/// Flutter-style undo/redo history stack. Pairs with a
/// `TextEditingController` (attached by `Input` at view-build time). The
/// controller owns the *current* value; this object owns the *history*.
/// Each call to `push` records a prior state (cleared on push, matching the
/// standard "redo branch is abandoned when the user types again"
/// convention).
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

