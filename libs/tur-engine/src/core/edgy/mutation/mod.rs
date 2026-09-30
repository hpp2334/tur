pub mod event_arg;
pub mod handle;
pub mod queue;

pub use crate::core::js_runtime::js_value::IntoJsArgs;
pub use event_arg::{extract_mutation_from_opts, mutation_from_js};
pub use handle::MutationHandle;
pub use queue::{MutationPayload, PendingMutationInvocationQueue};

// The dual-shape payload impls, JS arm only (the native arm defaults to
// empty — the historical realm-free degradation). The payloads the rut
// rail consumes with REAL data (pointer positions, key events, scroll,
// animation ticks) hand-implement `MutationPayload` in their own modules
// with a `to_value_args` override.
crate::mutation_payload!(
    crate::builtin_plugins::gesture::PointerRegionEvent,
    crate::core::focus::FocusEvent,
    crate::core::focus::BlurEvent,
    crate::builtin_plugins::text::controller::events::InputEvent,
    crate::builtin_plugins::text::controller::events::CursorChangeEvent,
    crate::builtin_plugins::text::controller::events::SelectionChangeEvent,
    crate::builtin_plugins::text::controller::events::CompositionStartEvent,
    crate::builtin_plugins::text::controller::events::CompositionUpdateEvent,
    crate::builtin_plugins::text::controller::events::CompositionEndEvent,
);
crate::mutation_payload!(());
