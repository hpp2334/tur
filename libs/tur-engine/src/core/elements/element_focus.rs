pub trait ElementOnFocus: 'static {
    fn on_focus(&mut self) {}
    fn on_blur(&mut self) {}
}
