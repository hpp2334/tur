use crate::core::element::NodeId;
use tur_shared::Offset;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComposedGestureEventKind {
    Click,
}

/// Result of engine-side multi-click classification. Produced by
/// `GestureEventComposer::classify_click` from a running history of recent
/// pointer-downs — two clicks within the time/distance window yield `Double`,
/// three in a row yield `Triple`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClickKind {
    Single,
    Double,
    Triple,
}

/// Thresholds for multi-click detection (match typical OS / browser defaults).
const MULTI_CLICK_MAX_INTERVAL_MS: u64 = 500;
const MULTI_CLICK_MAX_DISTANCE_PX: f64 = 5.0;

pub struct GestureEventComposer {
    pointer_down_target: Option<NodeId>,
    /// The full hit-path captured at pointer-down time. Used to route
    /// subsequent move/up events to the same set of elements regardless of
    /// where the pointer moves during the drag — standard "gesture capture"
    /// semantics so a draggable element keeps receiving events even if the
    /// pointer leaves its bounds.
    pointer_down_path: Vec<NodeId>,
    is_tracking_drag: bool,
    /// Bounded history of recent pointer-downs (time_ms, position) used by
    /// `classify_click` to detect double / triple clicks. Only the most
    /// recent 2 entries are kept (older entries can't contribute to a
    /// triple).
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
        target: Option<NodeId>,
        path: Vec<NodeId>,
    ) {
        self.pointer_down_target = target;
        self.pointer_down_path = path;
        self.is_tracking_drag = target.is_some();
    }

    pub fn pointer_down_target(&self) -> Option<NodeId> {
        self.pointer_down_target
    }

    pub fn pointer_down_path(&self) -> &[NodeId] {
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

    /// Compare a fresh pointer-down at `position` / `now_ms` against the
    /// recent click history to decide whether this counts as a single,
    /// double, or triple click. A "double" requires the previous click to
    /// have landed within `MULTI_CLICK_MAX_INTERVAL_MS` and
    /// `MULTI_CLICK_MAX_DISTANCE_PX`; a "triple" requires two prior clicks
    /// each satisfying the same window against their predecessor.
    ///
    /// History is updated in place: a click outside the window resets the
    /// history to `[this_click]`; a click inside extends it (capped at 2
    /// entries so anything older than the triple predecessor is dropped).
    pub fn classify_click(&mut self, position: Offset, now_ms: u64) -> ClickKind {
        let in_window = |prev: &(u64, Offset)| -> bool {
            let dt = now_ms.saturating_sub(prev.0);
            let dx = position.x - prev.1.x;
            let dy = position.y - prev.1.y;
            dt <= MULTI_CLICK_MAX_INTERVAL_MS
                && (dx * dx + dy * dy).sqrt() <= MULTI_CLICK_MAX_DISTANCE_PX
        };

        // If every recent entry is still in the window, this click extends
        // the streak. Otherwise, the streak breaks — start a new history.
        let streak_continues = !self.click_history.is_empty()
            && self.click_history.iter().all(in_window);

        if streak_continues {
            self.click_history.push((now_ms, position));
        } else {
            self.click_history.clear();
            self.click_history.push((now_ms, position));
        }

        // Keep at most 2 prior entries — anything older than the triple
        // predecessor can never contribute again.
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
