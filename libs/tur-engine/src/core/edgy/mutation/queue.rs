use boa_engine::{Context, JsValue};

use crate::core::edgy::reactive::Mutation;
use crate::core::edgy::value::Value;
use crate::core::js_runtime::js_value::IntoJsArgs;

use super::handle::MutationHandle;

// ---------------------------------------------------------------------------
// PendingMutationInvocationQueue — the buffer of pending MutationHandle
// invocations.
//
// Elements/controllers/handlers call `push(mutation, event)` at event time;
// the flush loop drains it and invokes each mutation via the reactive store
// (prepending the `{get, set}` context object). No `NodeId` is needed:
// a mutation is a self-contained `Mutation` handle, so dispatch is resolved at push
// time, not flush time.
//
// Payloads cross in BOTH shapes: JS-shaped (the historical `IntoJsArgs`
// path, converted under the realm) and — since the rut rail — native
// [`Value`] args that need no realm at all. A payload opts into the native
// crossing by implementing [`MutationPayload::to_value_args`]; every other
// payload keeps the historical realm-free degradation (empty args).
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

/// The dual-shape payload trait: JS-shaped args for the JS rail, native
/// args (default: empty — the historical realm-free degradation) for the
/// rut rail. Implemented for every callback payload via [`mutation_payload!`]
/// (JS arm only) or by hand (both arms).
pub trait MutationPayload: 'static {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue>;
    fn to_value_args(&self) -> Vec<Value> {
        Vec::new()
    }
}

/// Implement [`MutationPayload`] for a payload type, delegating the JS arm
/// to its existing [`IntoJsArgs`] impl.
#[macro_export]
macro_rules! mutation_payload {
    ($($t:ty),* $(,)?) => {
        $(
            impl $crate::core::edgy::mutation::MutationPayload for $t {
                fn to_js_args(&self, ctx: &mut ::boa_engine::Context) -> Vec<::boa_engine::JsValue> {
                    $crate::core::js_runtime::js_value::IntoJsArgs::to_js_args(self, ctx)
                }
            }
        )*
    };
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

    pub fn push<E: IntoJsArgs + MutationPayload>(&mut self, mutation: MutationHandle<E>, event: E) {
        self.0.push(PendingMutationInvocation {
            mutation: mutation.mutation(),
            args: Box::new(event),
        });
    }

    pub fn drain(&mut self) -> Vec<PendingMutationInvocation> {
        std::mem::take(&mut self.0)
    }
}
