#[derive(Clone, Debug, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyEventType {
    Down,
    Up,
}

#[derive(Clone, Debug)]
pub struct AppKeyEvent {
    pub key: String,
    pub code: String,
    pub modifiers: Modifiers,
    pub event_type: KeyEventType,
}
