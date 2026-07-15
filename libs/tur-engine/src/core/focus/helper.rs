use crate::core::element::{ElementKind, ElementNodeId};
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

/// True if the currently-focused element is an editable text element.
/// Used by the embedder (e.g. tur-wasm) to manage IME state — focusing the
/// hidden `<textarea>` and positioning it at the caret.
pub fn focused_is_editable(tree: &NodeTreeData, focus_manager: &FocusManager) -> bool {
    static EDITABLE_TEXT_KIND: std::sync::LazyLock<ElementKind> =
        std::sync::LazyLock::new(|| ElementKind::new("tur_editable_text"));
    let Some(focused_id) = focus_manager.focused() else {
        return false;
    };
    let Some(node) = tree.get_element(focused_id) else {
        return false;
    };
    let Some(ref element) = node.element else {
        return false;
    };
    element.kind() == *EDITABLE_TEXT_KIND
}
