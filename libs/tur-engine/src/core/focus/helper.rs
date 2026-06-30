use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::focus::FocusManager;

pub fn find_focusable_in_path(tree: &NodeTreeData, path: &[ElementNodeId]) -> Option<ElementNodeId> {
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

/// True if the currently-focused element is an `EditableTextElement`.
/// Used to drive caret-blink redraws on idle frames and to expose IME state
/// to the embedder.
pub fn focused_is_editable(tree: &NodeTreeData, focus_manager: &FocusManager) -> bool {
    use crate::elements::EditableTextElement;
    let Some(focused_id) = focus_manager.focused() else {
        return false;
    };
    let Some(node) = tree.get_element(focused_id) else {
        return false;
    };
    let Some(ref element) = node.element else {
        return false;
    };
    element.cast::<EditableTextElement>().is_some()
}
