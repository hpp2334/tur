use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::render::Canvas;
use crate::core::resource::{ImageResource, ResourceId, ResourceMap};
use crate::core::widget::{PropValue, Val};

pub struct PaintContext<'a> {
    tree: &'a ElementTree,
    resource_map: &'a ResourceMap,
    focused_node_id: Option<ElementNodeId>,
    current_node_id: Option<ElementNodeId>,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(
        tree: &'a ElementTree,
        focused_node_id: Option<ElementNodeId>,
        current_node_id: ElementNodeId,
        resource_map: &'a ResourceMap,
    ) -> Self {
        PaintContext {
            tree,
            resource_map,
            focused_node_id,
            current_node_id: Some(current_node_id),
        }
    }

    pub fn paint_child(
        &self,
        child_id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, canvas, parent_offset, self.focused_node_id, self.resource_map);
    }

    pub fn is_focused(&self) -> bool {
        self.focused_node_id == self.current_node_id
    }

    pub fn get_image_resource(&self, id: ResourceId) -> Option<&ImageResource> {
        self.resource_map.get_image(id)
    }

    /// Resolve a `Val<T>` for painting.  Same as `LayoutContext::read_val`
    /// but without dep tracking (paint is a read-only pass that follows
    /// the already-computed layout).
    pub fn read_val<T: PropValue>(&self, val: &Val<T>) -> Option<T> {
        match val {
            Val::Static(t) => Some(t.clone()),
            Val::Reactive(atom) => {
                let store = self.tree.store.as_ref()?;
                let js = store.borrow().get_raw(atom.id());
                T::from_js(&js)
            }
        }
    }

    /// Convenience: resolve an `Option<Val<T>>`.
    pub fn read_val_opt<T: PropValue>(&self, val: Option<&Val<T>>) -> Option<T> {
        val.and_then(|v| self.read_val(v))
    }
}
