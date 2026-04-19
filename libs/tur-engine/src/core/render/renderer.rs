use crate::core::elements::ElementTree;

pub trait Renderer {
    fn render(&mut self, tree: &ElementTree);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
