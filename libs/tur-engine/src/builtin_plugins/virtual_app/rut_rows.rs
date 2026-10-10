//! The virtual-app family's `tur_host` pkg rows (via the pkg-extension
//! seam): the controller ops (`va_create_source` / `va_source_handle` /
//! `va_controller` / `va_destroy` / `va_status` / `va_error` /
//! `va_app_atom` / `va_bind_atom` / `va_app_set` / `va_app_clear`) and the
//! VirtualApp element spec — the rut twin of `createModuleSource` /
//! `createVirtualAppController` / `VirtualAppView`.
//!
//! Sources cross as ids: `va_create_source` registers a STRING-authored
//! source (the rut parent authors it); `va_source_handle` lifts a
//! Rust-registered source id (e.g. minted by the playground's
//! `pg_compile`) into the handle opaque — Rust-defined sources enter the
//! rut realm as plain `u64`s (store values, intent args, entry params).
//! The spawned child is a full engine instance. Status rides the same
//! reactive rail the JS controller exposes (`status$`); rut reads it via
//! `va_status`.

use std::rc::Rc;

use crate::builtin_plugins::virtual_app::element::VirtualAppView;
use crate::builtin_plugins::virtual_app::state::{VirtualControllerRef, VirtualState};
use crate::core::edgy::value::Value;
use crate::core::rut_runtime::{RutHandles, RutView};
use crate::core::view::Val;

use rut_vm::Opaque;

/// The pkg-extension payload: decl rows at compile time, bodies at boot.
pub(crate) fn install_ext(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    install_decl(cx);
    let Some(handles) = cx.handles else {
        return; // the compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    install(&mut *cx.pkg, &handles);
}

/// Declare the virtual-app rows (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        row("va_create_source", vec![TY_STR], TY_OPAQUE),
        row("va_source_handle", vec![TY_U64], TY_OPAQUE),
        row("va_controller", vec![TY_OPAQUE], TY_OPAQUE),
        row("va_new", vec![], TY_OPAQUE),
        row("va_controller_set", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("va_bind", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("va_size", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        row("va_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("va_build", vec![TY_OPAQUE], TY_OPAQUE),
        row("va_destroy", vec![TY_OPAQUE], TY_NIL),
        row("va_status", vec![TY_OPAQUE], TY_STR),
        row("va_error", vec![TY_OPAQUE], TY_STR),
        row("va_status_atom", vec![TY_OPAQUE], TY_U64),
        row("va_app_atom", vec![], TY_U64),
        row("va_bind_atom", vec![TY_OPAQUE], TY_U64),
        row("va_app_set", vec![TY_U64, TY_OPAQUE], TY_NIL),
        row("va_app_clear", vec![TY_U64], TY_NIL),
    ]);
}

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The module-source opaque (the string never crosses the API again).
pub struct RutSource(pub u64);

/// The controller opaque (the stable base id — the same identity the JS
/// controller handle carries).
pub struct RutController(pub u64);

/// The VirtualApp element spec (`app` is either a static controller
/// binding or a reactive atom — the JS `app$` twin).
pub(crate) struct VaSpec {
    app: Option<Val<VirtualControllerRef>>,
    width: Option<Val<f64>>,
    height: Option<Val<f64>>,
    query_key: Option<Vec<String>>,
}

/// The shared `VirtualState` (the register-phase plugin state).
fn state(handles: &RutHandles) -> Result<Rc<VirtualState>, rut_vm::Trap> {
    handles
        .inst
        .plugin_state::<VirtualState>()
        .ok_or_else(|| {
            rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                "virtual-app plugin not registered on this instance",
            )
        })
}

fn controller_value(base: u64) -> Value {
    Value::opaque(std::rc::Rc::new(VirtualControllerRef(base)) as std::rc::Rc<dyn std::any::Any>)
}

/// Install the virtual-app-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // va_create_source(src) -> opaque — register a string-authored child
    // module source (the `createModuleSource` twin).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_create_source", (&str,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, src: &str| {
        let id = state(&h)?.create_source(std::sync::Arc::from(src.to_string()));
        Ok(Opaque::alloc(vm, RutSource(id))?.handle().clone())
    });

    // va_source_handle(id) -> opaque — lift a Rust-registered source id
    // (e.g. the u64 `pg_compile` answers) into the source handle.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_source_handle", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64| {
        state(&h)?
            .resolve_source(id)
            .ok_or_else(|| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "va_source_handle: unknown source"))?;
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
            .inst
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

    // ---- the VirtualApp element spec -------------------------------------
    rut_vm::pkg_fn!(pkg, "va_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        let spec = VaSpec { app: None, width: None, height: None, query_key: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    // Static controller binding.
    rut_vm::pkg_fn!(pkg, "va_controller_set", (Opaque<VaSpec>, Opaque<RutController>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<VaSpec>, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        b.with_mut(vm, |_vm, s| s.app = Some(Val::Static(VirtualControllerRef(base))))
    });
    // Reactive controller binding (unbind destroys unless keepAlive).
    rut_vm::pkg_fn!(pkg, "va_bind", (Opaque<VaSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<VaSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| {
            s.app = Some(Val::Reactive(crate::core::edgy::reactive::Readable::Source(
                crate::core::edgy::reactive::Source::<VirtualControllerRef>::from_id(
                    crate::core::edgy::reactive::AtomId(atom as u32),
                ),
            )));
        })
    });
    rut_vm::pkg_fn!(pkg, "va_size", (Opaque<VaSpec>, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<VaSpec>, w: f64, hh: f64| {
        b.with_mut(vm, |_vm, s| {
            s.width = if w > 0.0 { Some(Val::Static(w)) } else { None };
            s.height = if hh > 0.0 { Some(Val::Static(hh)) } else { None };
        })
    });
    rut_vm::pkg_fn!(pkg, "va_qkey", (Opaque<VaSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<VaSpec>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_build", (Opaque<VaSpec>,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, b: Opaque<VaSpec>| {
        let st = state(&h)?;
        let view = b.with(|s| {
            Rc::new(VirtualAppView {
                state: st.clone(),
                app: s.app.clone(),
                background: None,
                width: s.width.clone(),
                height: s.height.clone(),
                query_key: s.query_key.clone(),
                fallback: None,
                error_view: None,
            }) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // va_app_atom() -> atom — mint an EMPTY (Nil) controller-ref source
    // (the reactive `app$` crossing with nothing bound yet; the decode
    // fails and the element stays unbound until a `va_app_set`).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_app_atom", () -> u64, move |_vm: &mut rut_vm::interp::Vm| {
        let atom = h
            .store
            .bridge()
            .decl_source::<crate::core::edgy::Value>(crate::core::edgy::Value::Nil)
            .id()
            .0 as u64;
        Ok(atom)
    });
    // va_bind_atom(ctrl) -> atom — mint a source atom seeded with the
    // controller ref (the reactive `app$` crossing: Nil clears the binding,
    // a fresh controller re-binds).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_bind_atom", (Opaque<RutController>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        let atom = h
            .store
            .bridge()
            .decl_source::<crate::core::edgy::Value>(controller_value(base))
            .id()
            .0 as u64;
        Ok(atom)
    });
    // va_app_set(atom, ctrl) / va_app_clear(atom) — bind / unbind.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_app_set", (u64, Opaque<RutController>) -> (), move |_vm: &mut rut_vm::interp::Vm, atom: u64, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        h.store
            .bridge()
            .set_source(
                crate::core::edgy::reactive::Source::<crate::core::edgy::Value>::from_id(
                    crate::core::edgy::reactive::AtomId(atom as u32),
                ),
                controller_value(base),
            )
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("va_app_set: {e}")))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_app_clear", (u64,) -> (), move |_vm: &mut rut_vm::interp::Vm, atom: u64| {
        h.store
            .bridge()
            .set_source(
                crate::core::edgy::reactive::Source::<crate::core::edgy::Value>::from_id(
                    crate::core::edgy::reactive::AtomId(atom as u32),
                ),
                crate::core::edgy::Value::Nil,
            )
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("va_app_clear: {e}")))
    });

    // va_status_atom(ctrl) -> atom — the controller's status rail as a
    // reactive atom id (a watchable edgy source: `rs_watch` it and the
    // idle→spawning→running/error transitions deliver as intents).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "va_status_atom", (Opaque<RutController>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutController>| {
        let base = ctrl.with(|c| c.0)?;
        let st = state(&h)?;
        let record = st
            .record(base)
            .ok_or_else(|| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "va_status_atom: unknown controller"))?;
        Ok(record.status.id().0 as u64)
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
