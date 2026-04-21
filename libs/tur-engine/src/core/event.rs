use tur_shared::Offset;

pub enum RawAppEvent {
    PointerDown { position: Offset },
    PointerUp { position: Offset },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    PointerDown,
    PointerUp,
    Click,
}

pub enum AppEvent {
    PointerDown(AppPointerEvent),
    PointerUp(AppPointerEvent),
    Click(AppPointerEvent),
}

pub struct AppPointerEvent {
    pub position: Offset,
}
