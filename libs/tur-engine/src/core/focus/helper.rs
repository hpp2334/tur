use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::ElementTree;

pub fn find_focusable_in_path(tree: &ElementTree, path: &[NodeId]) -> Option<NodeId> {
    for &id in path {
        if let Some(node) = tree.get_element(ElementNodeId::new(id.as_u64())) {
            if let Some(ref element) = node.element {
                if element.has_focus() {
                    return Some(id);
                }
            }
        }
    }
    None
}
