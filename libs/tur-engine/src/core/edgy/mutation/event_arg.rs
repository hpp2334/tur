use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::edgy::reactive::Mutation;
use crate::core::js_runtime::js_value::{FromJs, IntoJsArgs};

use super::handle::MutationHandle;

// ---------------------------------------------------------------------------
// JS extraction helpers — the controller-side analogue of `prop_mutation` for
// specs. Both paths produce a `MutationHandle<E>` from an atom handle. The
// [`IntoJsArgs`] trait itself lives in `core::js_value` alongside `FromJs` /
// `IntoJs`.
// ---------------------------------------------------------------------------

pub fn mutation_from_js<E: IntoJsArgs>(v: &JsValue) -> Option<MutationHandle<E>> {
    Mutation::from_js(v).ok().map(MutationHandle::new)
}

pub fn extract_mutation_from_opts<E: IntoJsArgs>(
    opts: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<MutationHandle<E>> {
    use boa_engine::js_string;
    let v = opts.get(js_string!(key), ctx).ok()?;
    mutation_from_js(&v)
}
