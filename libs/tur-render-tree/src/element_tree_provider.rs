use tur_trait::ElementNodeId;

use crate::RenderObject;

pub trait ElementTreeProvider {
    fn root_id(&self) -> Option<ElementNodeId>;
    fn children_of(&self, id: ElementNodeId) -> Vec<ElementNodeId>;
    fn render_object_for(&self, id: ElementNodeId) -> Box<dyn RenderObject>;
}
