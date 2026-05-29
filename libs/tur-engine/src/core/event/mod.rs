pub mod queue;

use crate::core::element::ElementNodeId;
use crate::core::keyboard::AppKeyEvent;
use tur_shared::Offset;

pub enum AppEvent {
    Resize {
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    },
    Gesture(AppGestureEvent),
    Wheel {
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
    ScrollOverscroll {
        source_id: ElementNodeId,
        delta: f64,
    },
    Key(AppKeyEvent),
    Ime(AppImeEvent),
    RequestDraw,
}

pub enum AppGestureEvent {
    PointerDown { position: Offset },
    PointerUp { position: Offset },
    PointerMove { position: Offset },
    Wheel { delta_x: f64, delta_y: f64, position: Offset },
}

#[derive(Clone, Debug)]
pub enum AppImeEvent {
    CompositionStart,
    CompositionUpdate {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    CompositionEnd {
        text: String,
    },
}
