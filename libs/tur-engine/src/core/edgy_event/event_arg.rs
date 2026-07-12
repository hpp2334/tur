use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::js_value::{FromJs, IntoJsArgs};
use crate::core::reactive::Mutation;

use super::mutation::EdgyMutation;

// ---------------------------------------------------------------------------
// JS extraction helpers — the controller-side analogue of `prop_mutation` for
// specs. Both paths produce an `EdgyMutation<E>` from an atom handle. The
// [`IntoJsArgs`] trait itself lives in `core::js_value` alongside `FromJs` /
// `IntoJs`.
// ---------------------------------------------------------------------------

pub fn edgy_mutation_from_js<E: IntoJsArgs>(v: &JsValue) -> Option<EdgyMutation<E>> {
    Mutation::from_js(v).ok().map(EdgyMutation::new)
}

pub fn extract_mutation_from_opts<E: IntoJsArgs>(
    opts: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<EdgyMutation<E>> {
    use boa_engine::js_string;
    let v = opts.get(js_string!(key), ctx).ok()?;
    edgy_mutation_from_js(&v)
}
