use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::reactive::extract_mutation;

use super::mutation::EdgyMutation;

// ---------------------------------------------------------------------------
// EventArg — convert an event payload to its JS callback arguments.
//
// Implementations live alongside their event structs in each event's owning
// module (e.g. keyboard events in core/keyboard, scroll events in
// core/scroll, pointer events in elements/pointer_interact).
//
// Object-safe so the queue can store `Box<dyn EventArg>`.
// ---------------------------------------------------------------------------

pub trait EventArg: 'static {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue>;
}

/// No-arg callbacks (lifecycle hooks: onMounted / onUpdated / beforeDestroy).
impl EventArg for () {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// JS extraction helpers — the controller-side analogue of `prop_mutation` for
// specs. Both paths produce an `EdgyMutation<E>` from an atom handle.
// ---------------------------------------------------------------------------

pub fn edgy_mutation_from_js<E: EventArg>(v: &JsValue) -> Option<EdgyMutation<E>> {
    extract_mutation(v).map(EdgyMutation::new)
}

pub fn extract_mutation_from_opts<E: EventArg>(
    opts: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<EdgyMutation<E>> {
    use boa_engine::js_string;
    let v = opts.get(js_string!(key), ctx).ok()?;
    edgy_mutation_from_js(&v)
}
