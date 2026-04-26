use tur_shared::Offset;

pub enum AppEvent {
    Resize {
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    },
    Gesture(AppGestureEvent),
    RequestDraw,
}

pub enum AppGestureEvent {
    PointerDown { position: Offset },
    PointerUp { position: Offset },
}
