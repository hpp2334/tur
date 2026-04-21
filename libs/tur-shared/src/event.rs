use crate::Offset;

pub enum PointerPhase {
    Down,
    Up,
    Move,
}

pub struct PointerEvent {
    pub phase: PointerPhase,
    pub position: Offset,
    pub timestamp: f64,
}
