pub mod queue;

use crate::core::keyboard::AppKeyEvent;
use tur_shared::Offset;

pub enum AppEvent {
    Resize {
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    },
    Gesture(AppGestureEvent),
    Key(AppKeyEvent),
    RequestDraw,
}

pub enum AppGestureEvent {
    PointerDown { position: Offset },
    PointerUp { position: Offset },
    PointerMove { position: Offset },
}
