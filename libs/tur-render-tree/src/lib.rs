pub mod render_node;
pub mod render_tree;

pub use render_node::*;
pub use render_tree::*;

pub trait Renderer {
    fn render(&mut self, tree: &RenderTree);
}
