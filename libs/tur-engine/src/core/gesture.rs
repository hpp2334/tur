use crate::core::element::ElementNodeId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComposedGestureEventKind {
    Click,
}

pub struct GestureEventComposer {
    pointer_down_target: Option<ElementNodeId>,
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
        }
    }

    pub fn on_pointer_down(&mut self, target: Option<ElementNodeId>) {
        self.pointer_down_target = target;
    }

    pub fn pointer_down_target(&self) -> Option<ElementNodeId> {
        self.pointer_down_target
    }

    pub fn on_pointer_up(&mut self, click_eligible: bool) -> Option<ComposedGestureEventKind> {
        let down_target = self.pointer_down_target.take();
        if down_target.is_some() && click_eligible {
            Some(ComposedGestureEventKind::Click)
        } else {
            None
        }
    }
}
