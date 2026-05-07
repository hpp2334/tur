use std::collections::HashMap;

use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::render::Renderer;
use crate::core::resource::ResourceMap;

pub struct NoopRenderer;

impl Default for NoopRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl NoopRenderer {
    pub fn new() -> Self {
        NoopRenderer
    }
}

impl Renderer for NoopRenderer {
    fn render(&mut self, tree: &ElementTree, _focused_node_id: Option<ElementNodeId>, _resource_map: &ResourceMap) {
        let root_id = match tree.root_id() {
            Some(id) => id,
            None => {
                tracing::debug!("noop-renderer: empty element tree");
                return;
            }
        };

        let mut counts: HashMap<&str, usize> = HashMap::new();
        let max_depth = collect_stats(tree, root_id, Offset::ZERO, 0, &mut counts);

        let total: usize = counts.values().sum();
        tracing::debug!(
            "noop-renderer: {} nodes, max depth {}, breakdown: {:?}",
            total,
            max_depth,
            counts
        );
    }
}

fn collect_stats(
    tree: &ElementTree,
    id: ElementNodeId,
    parent_offset: Offset,
    depth: usize,
    counts: &mut HashMap<&str, usize>,
) -> usize {
    let node = match tree.get(id) {
        Some(n) => n,
        None => return depth,
    };

    let type_name = node
        .element
        .as_ref()
        .map(|e| e.type_name())
        .unwrap_or("tur_container");
    let absolute_offset = parent_offset + node.computed_layout.offset;
    tracing::trace!(
        "noop-renderer: {} node {} at ({:.1}, {:.1}) size ({:.1}, {:.1}) depth {}",
        type_name,
        node.id,
        absolute_offset.x,
        absolute_offset.y,
        node.computed_layout.size.width,
        node.computed_layout.size.height,
        depth,
    );

    *counts.entry(type_name).or_insert(0) += 1;

    let mut child_max = depth;
    for &child_id in &node.children {
        let d = collect_stats(tree, child_id, absolute_offset, depth + 1, counts);
        if d > child_max {
            child_max = d;
        }
    }
    child_max
}
