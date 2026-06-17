use crate::core::reactive::AtomId;

use super::event_arg::EventArg;
use super::mutation::EdgyMutation;

// ---------------------------------------------------------------------------
// PendingMutationInvocationQueue — the buffer of pending EdgyMutation
// invocations.
//
// Elements/controllers/handlers call `push(mutation, event)` at event time;
// the flush loop drains it and invokes each mutation via the reactive store
// (prepending the `{get, set}` context object). No `ElementNodeId` is needed:
// a mutation is a self-contained `AtomId`, so dispatch is resolved at push
// time, not flush time.
// ---------------------------------------------------------------------------

pub struct PendingMutationInvocationQueue(Vec<PendingMutationInvocation>);

impl std::fmt::Debug for PendingMutationInvocationQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingMutationInvocationQueue")
            .field("len", &self.0.len())
            .finish()
    }
}

pub(crate) struct PendingMutationInvocation {
    pub(crate) atom_id: AtomId,
    pub(crate) args: Box<dyn EventArg>,
}

impl Default for PendingMutationInvocationQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingMutationInvocationQueue {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push<E: EventArg>(&mut self, mutation: EdgyMutation<E>, event: E) {
        self.0.push(PendingMutationInvocation {
            atom_id: mutation.id(),
            args: Box::new(event),
        });
    }

    pub(crate) fn drain(&mut self) -> Vec<PendingMutationInvocation> {
        std::mem::take(&mut self.0)
    }
}
