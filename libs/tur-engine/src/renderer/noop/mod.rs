use std::collections::HashMap;

use crate::core::layout::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::render::{NullCanvas, Renderer};
use crate::core::resource::ResourceMap;
use crate::core::shell::PaintShell;

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
    fn render(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        resource_map: &ResourceMap,
        shell: PaintShell<'_>,
    ) {
        let root_id = match tree.root_element_id() {
            Some(id) => id,
            None => {
                tracing::debug!("noop-renderer: empty element tree");
                return;
            }
        };

        // Drive the paint walk against a null canvas so paint-time outputs
        // (cursor resolution) still happen even though nothing is drawn.
        let mut null = NullCanvas;
        tree.paint(&mut null, focused_node_id, resource_map, shell);

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
    tree: &NodeTreeData,
    id: ElementNodeId,
    parent_offset: Offset,
    depth: usize,
    counts: &mut HashMap<&str, usize>,
) -> usize {
    let node = match tree.get_element(id) {
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
    let children = tree.children_of_element(id);
    for child_id in children {
        let d = collect_stats(tree, child_id, absolute_offset, depth + 1, counts);
        if d > child_max {
            child_max = d;
        }
    }
    child_max
}
