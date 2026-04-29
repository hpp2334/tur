use crate::core::focus::FocusEventType;

pub trait ElementOnFocus: 'static {
    fn on_focus_event(&mut self, _event_type: FocusEventType) {}
}
