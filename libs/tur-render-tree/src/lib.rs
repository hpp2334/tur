pub mod render_node;
pub mod render_tree;

pub use render_node::*;
pub use render_tree::*;

pub trait Renderer {
    fn render(&mut self, tree: &RenderTree);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
