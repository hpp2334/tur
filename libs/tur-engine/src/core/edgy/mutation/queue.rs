use crate::core::edgy::mutation::MutationHandle;
use crate::core::edgy::reactive::Mutation;
use crate::core::edgy::value::Value;

// ---------------------------------------------------------------------------
// PendingMutationInvocationQueue — the buffer of pending MutationHandle
// invocations.
//
// Elements/controllers/handlers call `push(mutation, event)` at event time;
// the flush loop drains it and invokes each mutation via the reactive store.
// No `NodeId` is needed: a mutation is a self-contained `Mutation` handle,
// so dispatch is resolved at push time, not flush time.
//
// Payloads cross as native [`Value`] args (via [`MutationPayload::
// to_value_args`]) — the rut rail's callbacks read them directly.
// ---------------------------------------------------------------------------

pub struct PendingMutationInvocationQueue(Vec<PendingMutationInvocation>);

impl std::fmt::Debug for PendingMutationInvocationQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingMutationInvocationQueue")
            .field("len", &self.0.len())
            .finish()
    }
}

pub struct PendingMutationInvocation {
    pub(crate) mutation: Mutation,
    pub(crate) args: Box<dyn MutationPayload>,
}

/// The payload trait: native args crossing into the mutation invocation.
/// Payloads with real data (pointer positions, key events, scroll, animation
/// ticks) hand-implement the override; every other payload defaults to empty
/// args (the callback reads its payload from closure captures).
pub trait MutationPayload: 'static {
    fn to_value_args(&self) -> Vec<Value> {
        Vec::new()
    }
}

/// A pre-encoded native-arg payload — the ctx.run rail's crossing (the
/// composition queues the invocation with its args).
pub struct ValueArgs(pub Vec<Value>);

impl MutationPayload for ValueArgs {
    fn to_value_args(&self) -> Vec<Value> {
        self.0.clone()
    }
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

    pub fn push<E: MutationPayload>(&mut self, mutation: MutationHandle<E>, event: E) {
        self.0.push(PendingMutationInvocation {
            mutation: mutation.mutation(),
            args: Box::new(event),
        });
    }

    pub fn drain(&mut self) -> Vec<PendingMutationInvocation> {
        std::mem::take(&mut self.0)
    }
}
