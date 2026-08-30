//! `tur:std` bridge fns for virtual apps — plain ctx-bound `FnEntry` fn
//! pointers. The instance's shared
//! [`Rc<VirtualState>`](super::state::VirtualState) rides the register-phase
//! plugin-state channel
//! ([`TurInstanceContext::plugin_state`]), reached via `extract_js_ctx` —
//! no closure captures, no `unsafe`. User args start at index 1 like every
//! other ctx-bound bridge fn.

use std::rc::Rc;
use std::sync::Arc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, js_string};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};
use crate::core::js_runtime::js_value::IntoJs;

use super::element::VirtualAppView;
use super::state::{JsWorkerPoolHandle, ModuleSourceHandle, VirtualState};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("createModuleSource", 2, tur_create_module_source as Ptr),
        (
            "createVirtualAppController",
            2,
            tur_create_virtual_app_controller as Ptr,
        ),
        ("VirtualAppView", 2, tur_virtual_app_view as Ptr),
        ("forWorkerPool", 2, tur_for_worker_pool as Ptr),
    ]
}

/// The instance's shared virtual-app state (defined by `install_virtual_app`
/// during register).
fn state(args: &[JsValue]) -> JsResult<Rc<VirtualState>> {
    extract_js_ctx(args)?
        .plugin_state::<VirtualState>()
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message(
                "virtual-app state not registered — TurStdPlugin must be registered \
                 on this instance",
            ))
        })
}

fn tur_create_module_source(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let state = state(args)?;
    let source = args
        .get_or_undefined(1)
        .as_string()
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("createModuleSource(source: string) — expected a source string"),
            )
        })?
        .to_std_string_escaped();
    let handle = state.register_source(Arc::from(source.as_str()));
    Ok(ModuleSourceHandle(handle).into_js(ctx))
}

fn tur_create_virtual_app_controller(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let state = js_ctx.plugin_state::<VirtualState>().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "virtual-app state not registered — TurStdPlugin must be registered \
                 on this instance",
        ))
    })?;
    let props = require_props_object(args, 1, ctx)?;
    let source_handle = {
        let mut p = crate::core::js_runtime::JsProps::new(&props, ctx);
        p.opaque::<ModuleSourceHandle>("source")
    }
    .ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "createVirtualAppController({ source }) — `source` must be a ModuleSourceHandle \
             from createModuleSource()",
        ))
    })?;
    let handle = source_handle
        .downcast_ref::<ModuleSourceHandle>()
        .expect("opaque::<ModuleSourceHandle> checked the payload")
        .0;
    let source = state.resolve_source(handle).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("unknown ModuleSourceHandle — sources are single-use per controller"),
        )
    })?;
    let pool = {
        let v = props.get(js_string!("pool"), ctx)?;
        if v.is_null() || v.is_undefined() {
            // Default: the auto-registered `"virtual"` pool.
            js_ctx
                .find_worker_pool(crate::core::virtual_app::DEFAULT_POOL)
                .ok_or_else(|| {
                    JsError::from(JsNativeError::typ().with_message(
                        "the default `virtual` worker pool is not registered on this runtime",
                    ))
                })?
        } else {
            let obj = v.as_object().ok_or_else(|| {
                JsError::from(
                    JsNativeError::typ()
                        .with_message("`pool` must be a WorkerPoolHandle from forWorkerPool()"),
                )
            })?;
            obj.downcast_ref::<JsWorkerPoolHandle>()
                .map(|h| h.0.clone())
                .ok_or_else(|| {
                    JsError::from(
                        JsNativeError::typ()
                            .with_message("`pool` must be a WorkerPoolHandle from forWorkerPool()"),
                    )
                })?
        }
    };
    let keep_alive = {
        let v = props.get(js_string!("keepAlive"), ctx)?;
        !v.is_null() && !v.is_undefined() && v.as_boolean().unwrap_or(false)
    };

    let base = state.create_controller(source, pool, keep_alive);

    // `destroy$` — the ONLY lifecycle action, a control mutation (the
    // `watch` `{ start$, stop$ }` convention: side effects ride the mutation
    // rail — serialized ordering within a frame; a destroy issued inside a
    // flush lands the status flip + view swap in the same fixed-point).
    let state_for_destroy = state.clone();
    let destroy = state.bridge.build_mutate(move |_b, _args, _boa| {
        state_for_destroy.destroy(base);
        Ok(JsValue::undefined())
    });

    Ok(state.controller_js_object(base, destroy, ctx))
}

fn tur_virtual_app_view(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let state = state(args)?;
    let props: JsObject = require_props_object(args, 1, ctx)?;
    let view = VirtualAppView::from_js(&props, ctx, state);
    Ok(wrap_view(Rc::new(view), ctx))
}

/// `forWorkerPool(name) -> WorkerPoolHandle` — resolve a registered worker
/// pool by name, eagerly (an unknown name throws right here, not as an
/// async spawn error later). The returned handle wraps the very
/// `WorkerPoolHandle` the embedder registered on the runtime builder, and
/// is the only value `createVirtualAppController({ pool })` accepts.
fn tur_for_worker_pool(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let name = args
        .get_or_undefined(1)
        .as_string()
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("forWorkerPool(name: string) — expected a pool name"),
            )
        })?
        .to_std_string_escaped();
    let Some(handle) = js_ctx.find_worker_pool(&name) else {
        let registered: Vec<&str> = js_ctx.worker_pools.iter().map(|p| p.name()).collect();
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "unknown worker pool `{name}` — registered pools: {}",
            registered.join(", ")
        ))));
    };
    Ok(JsWorkerPoolHandle(handle).into_js(ctx))
}
