use std::collections::HashSet;

use crate::core::element::ElementNodeId;

pub struct PointerRegionDiff {
    pub entered: Vec<ElementNodeId>,
    pub exited: Vec<ElementNodeId>,
}

pub struct PointerRegionTracker {
    tracked: HashSet<ElementNodeId>,
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
        hit_path: &[ElementNodeId],
        has_callbacks: impl Fn(ElementNodeId) -> bool,
    ) -> PointerRegionDiff {
        let new_set: HashSet<ElementNodeId> = hit_path
            .iter()
            .copied()
            .filter(|id| has_callbacks(*id))
            .collect();

        let entered: Vec<ElementNodeId> = new_set
            .iter()
            .copied()
            .filter(|id| !self.tracked.contains(id))
            .collect();

        let exited: Vec<ElementNodeId> = self
            .tracked
            .iter()
            .copied()
            .filter(|id| !new_set.contains(id))
            .collect();

        self.tracked = new_set;

        PointerRegionDiff { entered, exited }
    }
}
