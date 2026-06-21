use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::resource::ResourceMap;

pub trait Renderer {
    fn render(&mut self, tree: &ElementTree, focused_node_id: Option<ElementNodeId>, resource_map: &ResourceMap, now_ms: u64);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}
