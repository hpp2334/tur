use crate::core::element::ElementNodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusEventType {
    Focus,
    Blur,
}

pub struct FocusManager {
    focused_id: Option<ElementNodeId>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManager {
    pub fn new() -> Self {
        Self { focused_id: None }
    }

    pub fn focused(&self) -> Option<ElementNodeId> {
        self.focused_id
    }

    pub fn request_focus(&mut self, id: ElementNodeId) -> Option<ElementNodeId> {
        let old = self.focused_id;
        self.focused_id = Some(id);
        old
    }

    pub fn clear_focus(&mut self) -> Option<ElementNodeId> {
        self.focused_id.take()
    }

    pub fn is_focused(&self, id: ElementNodeId) -> bool {
        self.focused_id == Some(id)
    }
}
