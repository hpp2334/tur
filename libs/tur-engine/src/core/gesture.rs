use crate::core::element::ElementNodeId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComposedGestureEventKind {
    Click,
}

pub struct GestureEventComposer {
    pointer_down_target: Option<ElementNodeId>,
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
            is_tracking_drag: false,
        }
    }

    pub fn on_pointer_down(&mut self, target: Option<ElementNodeId>) {
        self.pointer_down_target = target;
        self.is_tracking_drag = target.is_some();
    }

    pub fn pointer_down_target(&self) -> Option<ElementNodeId> {
        self.pointer_down_target
    }

    pub fn is_tracking_drag(&self) -> bool {
        self.is_tracking_drag
    }

    pub fn on_pointer_move(&mut self) -> bool {
        self.is_tracking_drag
    }

    pub fn on_pointer_up(&mut self, click_eligible: bool) -> Option<ComposedGestureEventKind> {
        let down_target = self.pointer_down_target.take();
        self.is_tracking_drag = false;
        if down_target.is_some() && click_eligible {
            Some(ComposedGestureEventKind::Click)
        } else {
            None
        }
    }
}
