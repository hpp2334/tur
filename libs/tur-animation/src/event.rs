use boa_engine::{Context, JsValue};

use tur_engine::core::edgy::mutation::IntoJsArgs;

// ---------------------------------------------------------------------------
// Animation callback payloads — JS callback arguments for onTick / onEnd.
//
// `onTick(easedValue)` receives the eased progress in [0.0, 1.0].
// `onEnd()` receives no payload.
//
// Both are dispatched via `PendingMutationInvocationQueue` (the same mechanism
// used for keyboard/pointer/scroll events), so callbacks fire during the
// engine's flush loop after all `RefMut` borrows are released. This lets the
// callback safely read controller properties (`ctrl.status`, `ctrl.value`)
// without triggering boa's `BorrowError`.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct AnimationTickEvent(pub f64);

impl IntoJsArgs for AnimationTickEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(self.0)]
    }
}

#[derive(Clone, Copy)]
pub struct AnimationEndEvent;

impl IntoJsArgs for AnimationEndEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}
