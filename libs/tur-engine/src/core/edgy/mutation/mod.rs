pub mod handle;
pub mod queue;

pub use handle::MutationHandle;
pub use queue::{MutationPayload, PendingMutationInvocationQueue, ValueArgs};

// The payload impls for the event payloads the drain invokes with REAL
// native args (pointer positions, key events, scroll, animation ticks)
// hand-implement `MutationPayload::to_value_args` in their own modules.
