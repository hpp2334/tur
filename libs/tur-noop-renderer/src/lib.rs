use std::collections::HashMap;

use tur_render_tree::{RenderNodeId, RenderTree, Renderer};
use tur_trait::Offset;

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
    fn render(&mut self, tree: &RenderTree) {
        let root_id = match tree.root_id() {
            Some(id) => id,
            None => {
                tracing::debug!("noop-renderer: empty render tree");
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
    tree: &RenderTree,
    id: RenderNodeId,
    parent_offset: Offset,
    depth: usize,
    counts: &mut HashMap<&str, usize>,
) -> usize {
    let node = match tree.get(id) {
        Some(n) => n,
        None => return depth,
    };

    let type_name = node
        .object
        .as_ref()
        .map(|o| o.type_name())
        .unwrap_or("tur_container");
    let absolute_offset = parent_offset + node.computed_layout.offset;
    tracing::trace!(
        "noop-renderer: {} node {} at ({:.1}, {:.1}) size ({:.1}, {:.1}) depth {}",
        type_name,
        id.as_u64(),
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
