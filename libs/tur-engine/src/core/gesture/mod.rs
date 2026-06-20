use crate::core::element::ElementNodeId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComposedGestureEventKind {
    Click,
}

pub struct GestureEventComposer {
    pointer_down_target: Option<ElementNodeId>,
    /// The full hit-path captured at pointer-down time. Used to route
    /// subsequent move/up events to the same set of elements regardless of
    /// where the pointer moves during the drag — standard "gesture capture"
    /// semantics so a draggable element keeps receiving events even if the
    /// pointer leaves its bounds.
    pointer_down_path: Vec<ElementNodeId>,
    is_tracking_drag: bool,
}

impl Default for GestureEventComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureEventComposer {
    pub fn new() -> Self {
        Self {
            pointer_down_target: None,
            pointer_down_path: Vec::new(),
            is_tracking_drag: false,
        }
    }

    pub fn on_pointer_down(
        &mut self,
        target: Option<ElementNodeId>,
        path: Vec<ElementNodeId>,
    ) {
        self.pointer_down_target = target;
        self.pointer_down_path = path;
        self.is_tracking_drag = target.is_some();
    }

    pub fn pointer_down_target(&self) -> Option<ElementNodeId> {
        self.pointer_down_target
    }

    pub fn pointer_down_path(&self) -> &[ElementNodeId] {
        &self.pointer_down_path
    }

    pub fn is_tracking_drag(&self) -> bool {
        self.is_tracking_drag
    }

    pub fn on_pointer_move(&mut self) -> bool {
        self.is_tracking_drag
    }

    pub fn on_pointer_up(&mut self, click_eligible: bool) -> Option<ComposedGestureEventKind> {
        let down_target = self.pointer_down_target.take();
        self.pointer_down_path.clear();
        self.is_tracking_drag = false;
        if down_target.is_some() && click_eligible {
            Some(ComposedGestureEventKind::Click)
        } else {
            None
        }
    }
}
