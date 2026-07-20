use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::image_resource::ImageResourceMap;
use crate::core::shell::PaintShell;

pub trait Renderer {
    fn render(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
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
