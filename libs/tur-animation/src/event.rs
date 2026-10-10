use tur_engine::core::edgy::Value;
use tur_engine::core::edgy::mutation::MutationPayload;

// ---------------------------------------------------------------------------
// Animation callback payloads — the onTick / onEnd callback arguments.
//
// `onTick(easedValue)` receives the eased progress in [0.0, 1.0].
// `onEnd()` receives no payload.
//
// Both are dispatched via `PendingMutationInvocationQueue` (the same mechanism
// used for keyboard/pointer/scroll events), so callbacks fire during the
// engine's flush loop after all `RefMut` borrows are released. This lets the
// callback safely read controller state (`status`, `value`).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct AnimationTickEvent(pub f64);

impl MutationPayload for AnimationTickEvent {
    /// The eased progress.
    fn to_value_args(&self) -> Vec<Value> {
        vec![Value::Num(self.0)]
    }
}

#[derive(Clone, Copy)]
pub struct AnimationEndEvent;

impl MutationPayload for AnimationEndEvent {}
