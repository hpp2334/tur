use std::collections::HashSet;

use crate::core::element::NodeId;

pub struct PointerRegionDiff {
    pub entered: Vec<NodeId>,
    pub exited: Vec<NodeId>,
}

pub struct PointerRegionTracker {
    tracked: HashSet<NodeId>,
}

impl Default for PointerRegionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerRegionTracker {
    pub fn new() -> Self {
        Self {
            tracked: HashSet::new(),
        }
    }

    pub fn update(
        &mut self,
        hit_path: &[NodeId],
        has_callbacks: impl Fn(NodeId) -> bool,
    ) -> PointerRegionDiff {
        let new_set: HashSet<NodeId> = hit_path
            .iter()
            .copied()
            .filter(|id| has_callbacks(*id))
            .collect();

        let entered: Vec<NodeId> = new_set
            .iter()
            .copied()
            .filter(|id| !self.tracked.contains(id))
            .collect();

        let exited: Vec<NodeId> = self
            .tracked
            .iter()
            .copied()
            .filter(|id| !new_set.contains(id))
            .collect();

        self.tracked = new_set;

        PointerRegionDiff { entered, exited }
    }
}
