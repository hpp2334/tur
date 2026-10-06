//! The lifecycle family's `tur` host-pkg rows (via the pkg-extension
//! seam): the Lifecycle spec — mount/destroy intent callbacks around a
//! pre-built child (the rut twin of the JS `lifecycleView(fn)`).

use std::rc::Rc;

use crate::builtin_plugins::lifecycle::{LifecycleFactory, LifecycleView};
use crate::core::edgy::mutation::MutationHandle;
use crate::core::edgy::value::Value;
use crate::core::rut_runtime::{cb_entries, Intent, RutHandles, RutView};

use rut_vm::{Opaque, OpaqueRef};

/// The pkg-extension payload: decl rows at compile time, bodies at boot.
pub(crate) fn install_ext(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    install_decl(cx);
    let Some(handles) = cx.handles else {
        return; // the compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    install(&mut *cx.pkg, &handles);
}

/// Declare the lifecycle rows (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        row("lc_new", vec![], TY_OPAQUE),
        row("lc_on_mount", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lc_before_destroy", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("lc_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("lc_build", vec![TY_OPAQUE], TY_OPAQUE),
    ]);
}

use rut_core::types::{TY_NIL, TY_OPAQUE, TY_U64};

/// The lifecycle spec.
pub(crate) struct LcSpec {
    on_mounted: Option<MutationHandle<()>>,
    before_destroy: Option<MutationHandle<()>>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

/// Queue a no-payload lifecycle intent for the `before_destroy` fn rail
/// (the drain dispatches `(cb, id, 0, 1)` — the callback is the kit-sealed
/// fn box). `on_mount` rides the mutation rail (`lc_on_mount(spec, atom)`)
/// — the element enqueues the sealed mutation and the flush's mutation
/// pass invokes it with the ctx + the typed `MountEvent`.
fn lifecycle_mutation(handles: &Rc<RutHandles>, id: u64, cb: OpaqueRef) -> Option<MutationHandle<()>> {
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
        h.pending_calls.borrow_mut().push(Intent::Click {
            entry: cb_entries::CLICK,
            cb: cb.clone(),
            a: id,
            b: 0,
            seq: 1.0,
        });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// Install the lifecycle-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "lc_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        let spec = LcSpec { on_mounted: None, before_destroy: None, child: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    // lc_on_mount(spec, atom) — the sealed on-mount mutation (the element
    // enqueues it at the mount lifecycle point; the flush's mutation pass
    // invokes it with the ctx + the typed `MountEvent`).
    rut_vm::pkg_fn!(pkg, "lc_on_mount", (Opaque<LcSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LcSpec>, atom: u64| {
        let m = Some(MutationHandle::<()>::new(crate::core::rut_runtime::mutation_of(atom)));
        b.with_mut(vm, |_vm, s| s.on_mounted = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "lc_before_destroy", (Opaque<LcSpec>, OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<LcSpec>, cb: OpaqueRef| {
        let m = lifecycle_mutation(&h, 1, cb);
        b.with_mut(vm, |_vm, s| s.before_destroy = m)
    });
    rut_vm::pkg_fn!(pkg, "lc_child", (Opaque<LcSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LcSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "lc_build", (Opaque<LcSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<LcSpec>| {
        let view = b.with(|s| {
            let child = s.child.clone().expect("lc_build: no child");
            Rc::new(LifecycleView {
                factory: LifecycleFactory::Rut {
                    child,
                    on_mounted: s.on_mounted,
                    before_destroy: s.before_destroy,
                },
            }) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}
