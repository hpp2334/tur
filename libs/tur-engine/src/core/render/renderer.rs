use crate::core::element::NodeId;
use crate::core::elements::ElementTree;
use crate::core::resource::ResourceMap;
use crate::core::shell::PaintShell;

pub trait Renderer {
    fn render(
        &mut self,
        tree: &ElementTree,
        focused_node_id: Option<NodeId>,
        resource_map: &ResourceMap,
        shell: PaintShell<'_>,
    );

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}
