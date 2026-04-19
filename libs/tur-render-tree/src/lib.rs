pub mod element_tree_provider;
pub mod objects;
pub mod render_node;
pub mod render_object;
pub mod render_tree;

pub use element_tree_provider::*;
pub use objects::*;
pub use render_node::*;
pub use render_object::*;
pub use render_tree::*;

pub use tur_shared::{
    Axis, ComputedLayout, Constraints, CrossAxisAlignment, EdgeInsets, FlexDirection, FlexFit,
    MainAxisAlignment, Offset, Size, StackFit,
};

pub trait Renderer {
    fn render(&mut self, tree: &RenderTree);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
