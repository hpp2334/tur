//! The scroll family's `tur_host` pkg rows (via the pkg-extension seam):
//! the ScrollView spec — axis, one-shot initial offset (the JS controller's
//! `initialOffset` twin), query key, child.

use std::rc::Rc;

use crate::builtin_plugins::scroll::ScrollViewView;
use crate::core::layout::Axis;
use crate::core::rut_runtime::RutView;
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

/// Declare the scroll rows (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        row("scroll_new", vec![], TY_OPAQUE),
        row("scroll_axis", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("scroll_offset", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("scroll_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("scroll_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("scroll_build", vec![TY_OPAQUE], TY_OPAQUE),
    ]);
}

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The scroll spec.
pub(crate) struct ScrollSpec {
    axis: Option<Axis>,
    initial_offset: Option<f64>,
    query_key: Option<Vec<String>>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

/// Install the scroll-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, _handles: &std::rc::Rc<crate::core::rut_runtime::RutHandles>) {
    rut_vm::pkg_fn!(pkg, "scroll_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        let spec = ScrollSpec { axis: None, initial_offset: None, query_key: None, child: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    // scroll_axis(spec, AXIS_*) — the wheel-driven viewport's direction
    // (the JS `axis` prop twin; default vertical).
    rut_vm::pkg_fn!(pkg, "scroll_axis", (Opaque<ScrollSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ScrollSpec>, v: u64| {
        let axis = if v == 1 { Axis::Horizontal } else { Axis::Vertical };
        b.with_mut(vm, |_vm, s| s.axis = Some(axis))
    });
    // scroll viewport with a one-shot initial offset (the JS controller's
    // `initialOffset` twin — applied after the first content layout).
    rut_vm::pkg_fn!(pkg, "scroll_offset", (Opaque<ScrollSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ScrollSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.initial_offset = Some(v))
    });
    rut_vm::pkg_fn!(pkg, "scroll_child", (Opaque<ScrollSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ScrollSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "scroll_qkey", (Opaque<ScrollSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ScrollSpec>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "scroll_build", (Opaque<ScrollSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<ScrollSpec>| {
        let view = b.with(|s| {
            let child = s.child.clone().expect("scroll_build: no child");
            let mut view = ScrollViewView::new_rut(s.axis.map(Val::Static), child);
            view.initial_offset = s.initial_offset.map(Val::Static);
            view.query_key = s.query_key.clone();
            Rc::new(view) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}
