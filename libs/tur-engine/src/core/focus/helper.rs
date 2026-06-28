use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;

pub fn find_focusable_in_path(tree: &ElementTree, path: &[ElementNodeId]) -> Option<ElementNodeId> {
    for &id in path {
        if let Some(node) = tree.get_element(id) {
            if let Some(ref element) = node.element {
                if element.has_focus() {
                    return Some(id);
                }
            }
        }
    }
    None
}
