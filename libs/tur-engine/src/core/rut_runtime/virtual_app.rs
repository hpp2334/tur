//! C7 — lifecycle rows + virtual apps: `el_lifecycle` (mount/destroy
//! intent callbacks around a pre-built child) and the virtual-app rows —
//! `va_source` / `va_controller` / `el_virtual_app` / `va_destroy` — the
//! rut twin of `createModuleSource` / `createVirtualAppController` /
//! `VirtualAppView`.
//!
//! The child module source crosses as a STRING (the rut parent authors
//! it); the spawned child is a full engine instance (its own realm when
//! its module is JS — the PARENT stays rut/realm-free). Status rides the
//! same reactive rail the JS controller exposes (`status$`); rut reads it
//! via `va_status`.

use std::rc::Rc;

use crate::builtin_plugins::lifecycle::{LifecycleFactory, LifecycleView};
use crate::builtin_plugins::virtual_app::state::VirtualState;
use crate::builtin_plugins::virtual_app::element::VirtualAppView;
use crate::core::edgy::mutation::MutationHandle;
use crate::core::edgy::value::Value;
use crate::core::view::Val;
use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR};
use rut_vm::Opaque;

use super::{Intent, RutHandles, RutView};

/// The module-source opaque (the string never crosses the API again).
pub struct RutSource(pub u64);

/// The controller opaque (the stable base id — the same identity the JS
/// controller handle carries).
pub struct RutController(pub u64);

/// Declare the C7 rows on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    vec![
        (
            "el_lifecycle",
            vec![TY_STR, TY_STR, TY_OPAQUE],
            TY_OPAQUE,
        ),
        ("va_source", vec![TY_STR], TY_OPAQUE),
        ("va_controller", vec![TY_OPAQUE], TY_OPAQUE),
        (
            "el_virtual_app",
            vec![TY_OPAQUE, TY_F64, TY_F64],
            TY_OPAQUE,
        ),
        ("va_destroy", vec![TY_OPAQUE], TY_NIL),
        ("va_status", vec![TY_OPAQUE], TY_STR),
        ("va_error", vec![TY_OPAQUE], TY_STR),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// The shared `VirtualState` (the register-phase plugin state).
fn state(handles: &RutHandles) -> Result<Rc<VirtualState>, rut_vm::Trap> {
    handles
        .js_ctx
        .plugin_state::<VirtualState>()
        .ok_or_else(|| {
            rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                "virtual-app plugin not registered on this instance",
            )
        })
}

/// Queue a no-payload lifecycle intent (the drain dispatches
/// `(id, 0, 1)`).
fn lifecycle_mutation(handles: &Rc<RutHandles>, id: u64, cb: &str) -> Option<MutationHandle<()>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args, _boa| {
        h.pending_calls.borrow_mut().push(Intent::Click {
            name: name.clone(),
            a: id,
            b: 0,
            seq: 1.0,
        });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// Install the C7 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // el_lifecycle(on_mount, before_destroy, child) — the C7 rut twin of
    // `lifecycleView(fn)`: a pre-built child + intent mutations.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_lifecycle", (&str, &str, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, on_mount: &str, before_destroy: &str, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        // The lifecycle id: the child tree node id is unknown at author
        // time, so the callbacks receive the sequence (b=0) — modules
        // route by closure-free convention (the label atom id).
        let view = Rc::new(LifecycleView {
            factory: LifecycleFactory::Rut {
                child,
                on_mounted: lifecycle_mutation(&h, 0, on_mount),
                before_destroy: lifecycle_mutation(&h, 1, before_destroy),
            },
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // va_source(src) -> opaque — register the child module source.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_source", (&str,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, src: &str| {
        let id = state(&h)?.register_source(std::sync::Arc::from(src.to_string()));
        Ok(Opaque::alloc(vm, RutSource(id))?.handle().clone())
    });

    // va_controller(src) -> opaque — the lazy declaration (default pool).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_controller", (Opaque<RutSource>,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, src: Opaque<RutSource>| {
        let source_id = src.with(|s| s.0)?;
        let st = state(&h)?;
        let resolved = st
            .resolve_source(source_id)
            .ok_or_else(|| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "va_controller: unknown source"))?;
        let pool = h
            .js_ctx
            .find_worker_pool(crate::core::virtual_app::DEFAULT_POOL)
            .ok_or_else(|| {
                rut_vm::Trap::new(
                    rut_vm::TrapKind::Invalid,
                    "va_controller: the default `virtual` worker pool is not registered",
                )
            })?;
        let base = st.create_controller(resolved, pool, false, None);
        Ok(Opaque::alloc(vm, RutController(base))?.handle().clone())
    });

    // el_virtual_app(ctrl, w, h) -> opaque — the host element (a static
    // controller binding; the rut rows author the concrete controller).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_virtual_app", (Opaque<RutController>, f64, f64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutController>, w: f64, hh: f64| {
        let _ = &h;
        let base = ctrl.with(|c| c.0)?;
        let view = Rc::new(VirtualAppView {
            state: state(&h)?,
            app: Some(Val::Static(crate::builtin_plugins::virtual_app::state::VirtualControllerRef(base))),
            background: None,
            width: if w > 0.0 { Some(Val::Static(w)) } else { None },
            height: if hh > 0.0 { Some(Val::Static(hh)) } else { None },
            query_key: Some(vec!["rut".to_string(), "vapp".to_string()]),
            fallback: None,
            error_view: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // va_destroy(ctrl) — the destroy$ twin.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_destroy", (Opaque<RutController>,) -> (), move |_vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        state(&h)?.destroy(base);
        Ok(())
    });

    // va_status(ctrl) -> str — the reactive status rail read natively.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_status", (Opaque<RutController>,) -> String, move |_vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        let st = state(&h)?;
        let Some(record) = st.record(base) else {
            return Ok(String::new());
        };
        let v = h.store.read_value(record.status.id()).unwrap_or(Value::Nil);
        Ok(v.as_str().unwrap_or_default().to_string())
    });

    // va_error(ctrl) -> str — the error-message rail.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_error", (Opaque<RutController>,) -> String, move |_vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        let st = state(&h)?;
        let Some(record) = st.record(base) else {
            return Ok(String::new());
        };
        let v = h.store.read_value(record.error_msg.id()).unwrap_or(Value::Nil);
        Ok(v.as_str().unwrap_or_default().to_string())
    });
}
