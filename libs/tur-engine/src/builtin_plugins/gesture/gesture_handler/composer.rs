//! Gesture event composer — capture tracking + multi-click classification.
//!
//! Owned by `GestureSubsystem`. Tracks the pointer-down path (gesture
//! capture) so subsequent move/up events route to the same set of elements
//! regardless of where the pointer moves. Also classifies single, double,
//! and triple clicks from a running history of recent pointer-downs.

use crate::core::element::ElementNodeId;
use crate::core::layout::Offset;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClickKind {
    Single,
    Double,
    Triple,
}

const MULTI_CLICK_MAX_INTERVAL_MS: u64 = 500;
const MULTI_CLICK_MAX_DISTANCE_PX: f64 = 5.0;

pub struct GestureEventComposer {
    pointer_down_target: Option<ElementNodeId>,
    pointer_down_path: Vec<ElementNodeId>,
    is_tracking_drag: bool,
    click_history: Vec<(u64, Offset)>,
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
            click_history: Vec::new(),
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

    pub fn on_pointer_up(&mut self, click_eligible: bool) -> bool {
        let down_target = self.pointer_down_target.take();
        self.pointer_down_path.clear();
        self.is_tracking_drag = false;
        down_target.is_some() && click_eligible
    }

    pub fn classify_click(&mut self, position: Offset, now_ms: u64) -> ClickKind {
        let in_window = |prev: &(u64, Offset)| -> bool {
            let dt = now_ms.saturating_sub(prev.0);
            let dx = position.x - prev.1.x;
            let dy = position.y - prev.1.y;
            dt <= MULTI_CLICK_MAX_INTERVAL_MS
                && (dx * dx + dy * dy).sqrt() <= MULTI_CLICK_MAX_DISTANCE_PX
        };

        let streak_continues = !self.click_history.is_empty()
            && self.click_history.iter().all(in_window);

        if streak_continues {
            self.click_history.push((now_ms, position));
        } else {
            self.click_history.clear();
            self.click_history.push((now_ms, position));
        }

        if self.click_history.len() > 3 {
            let drop = self.click_history.len() - 3;
            self.click_history.drain(0..drop);
        }

        match self.click_history.len() {
            1 => ClickKind::Single,
            2 => ClickKind::Double,
            _ => ClickKind::Triple,
        }
    }
}
