pub mod event_arg;
pub mod mutation;
pub mod queue;

pub use crate::core::js_value::IntoJsArgs;
pub use event_arg::{edgy_mutation_from_js, extract_mutation_from_opts};
pub use mutation::EdgyMutation;
pub use queue::PendingMutationInvocationQueue;
