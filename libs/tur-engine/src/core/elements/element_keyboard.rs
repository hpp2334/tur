use crate::core::keyboard::AppKeyEvent;

pub enum KeyboardResult {
    NotHandled,
    Handled,
    NeedsDraw,
}

pub trait ElementOnKeyboard: 'static {
    fn on_keyboard_event(&mut self, event: &AppKeyEvent) -> KeyboardResult {
        let _ = event;
        KeyboardResult::NotHandled
    }
}
