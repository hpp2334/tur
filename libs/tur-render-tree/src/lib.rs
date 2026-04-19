pub mod objects;
pub mod render_tree;

pub use objects::*;
pub use render_tree::*;
pub use tur_trait::*;

pub trait Renderer {
    fn render(&mut self, tree: &RenderTree);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
