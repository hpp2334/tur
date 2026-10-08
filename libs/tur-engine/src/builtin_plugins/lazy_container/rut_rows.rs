//! The lazy-container families' `tur_host` pkg rows (via the pkg-extension
//! seam): LazyList / LazyGrid over a reactive count atom and an entry-fn
//! item builder (`entry fn(index: u64) -> opaque`, invoked through the
//! guarded VM face — the same flush-time-call law the Each family rides).

use std::rc::Rc;

use crate::builtin_plugins::lazy_container::item_builder::RutEntryBuilder;
use crate::builtin_plugins::lazy_container::{LazyGridView, LazyListView};
use crate::core::layout::Axis;
use crate::core::rut_runtime::{RutHandles, RutView, readable_of};
use crate::core::view::Val;

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

/// Declare the lazy rows (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        // lazy list
        row("lazy_list_new", vec![], TY_OPAQUE),
        row("lazy_builder", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("lazy_count", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lazy_axis", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lazy_overscan", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lazy_item_extent", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("lazy_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("lazy_build", vec![TY_OPAQUE], TY_OPAQUE),
        // lazy grid
        row("lazy_grid_new", vec![], TY_OPAQUE),
        row("lazy_grid_builder", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("lazy_grid_count", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lazy_grid_axis", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lazy_grid_overscan", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("lazy_grid_max_cross", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("lazy_grid_aspect", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("lazy_grid_item_extent", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("lazy_grid_spacing", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        row("lazy_grid_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("lazy_grid_build", vec![TY_OPAQUE], TY_OPAQUE),
    ]);
}

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The LazyList spec.
pub(crate) struct LazyListSpec {
    builder: Option<RutEntryBuilder>,
    count: Option<Val<u64>>,
    axis: Option<Axis>,
    overscan: Option<u64>,
    item_extent: Option<f64>,
    query_key: Option<Vec<String>>,
}

/// The LazyGrid spec.
pub(crate) struct LazyGridSpec {
    builder: Option<RutEntryBuilder>,
    count: Option<Val<u64>>,
    axis: Option<Axis>,
    overscan: Option<u64>,
    max_cross: Option<f64>,
    aspect: Option<f64>,
    item_extent: Option<f64>,
    cross_spacing: Option<f64>,
    main_spacing: Option<f64>,
    query_key: Option<Vec<String>>,
}

fn axis_of(v: u64) -> Axis {
    if v == 1 { Axis::Horizontal } else { Axis::Vertical }
}

fn qkey_of(key: &str) -> Vec<String> {
    key.split('/').map(str::to_string).collect()
}

/// Install the lazy-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // ---- lazy list ---------------------------------------------------------
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "lazy_list_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        let spec = LazyListSpec { builder: None, count: None, axis: None, overscan: None, item_extent: None, query_key: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "lazy_builder", (Opaque<LazyListSpec>, OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>, cb: OpaqueRef| {
        let _ = vm;
        let entry = RutEntryBuilder { cb, face: h.face.clone(), handles: h.clone() };
        b.with_mut(vm, |_vm, s| s.builder = Some(entry))
    });
    rut_vm::pkg_fn!(pkg, "lazy_count", (Opaque<LazyListSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| s.count = Some(Val::Reactive(readable_of::<u64>(atom))))
    });
    rut_vm::pkg_fn!(pkg, "lazy_axis", (Opaque<LazyListSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.axis = Some(axis_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "lazy_overscan", (Opaque<LazyListSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.overscan = Some(v))
    });
    rut_vm::pkg_fn!(pkg, "lazy_item_extent", (Opaque<LazyListSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.item_extent = if v > 0.0 { Some(v) } else { None })
    });
    rut_vm::pkg_fn!(pkg, "lazy_qkey", (Opaque<LazyListSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "lazy_build", (Opaque<LazyListSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyListSpec>| {
        let view = b.with(|s| {
            Rc::new(LazyListView::new_rut(
                s.builder.clone().expect("lazy_build: no item builder"),
                s.count.clone().expect("lazy_build: no count atom"),
                s.axis,
                Some(s.overscan.unwrap_or(0)),
                s.item_extent,
                s.query_key.clone(),
            )) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- lazy grid ----------------------------------------------------------
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "lazy_grid_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        let spec = LazyGridSpec { builder: None, count: None, axis: None, overscan: None, max_cross: None, aspect: None, item_extent: None, cross_spacing: None, main_spacing: None, query_key: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "lazy_grid_builder", (Opaque<LazyGridSpec>, OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, cb: OpaqueRef| {
        let _ = vm;
        let entry = RutEntryBuilder { cb, face: h.face.clone(), handles: h.clone() };
        b.with_mut(vm, |_vm, s| s.builder = Some(entry))
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_count", (Opaque<LazyGridSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| s.count = Some(Val::Reactive(readable_of::<u64>(atom))))
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_axis", (Opaque<LazyGridSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.axis = Some(axis_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_overscan", (Opaque<LazyGridSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.overscan = Some(v))
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_max_cross", (Opaque<LazyGridSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.max_cross = Some(v))
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_aspect", (Opaque<LazyGridSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.aspect = if v > 0.0 { Some(v) } else { None })
    });
    // lazy_grid_item_extent — the FIXED main-axis cell extent (the boa
    // `mainAxisExtent`): overrides the aspect math (cell_main = extent).
    rut_vm::pkg_fn!(pkg, "lazy_grid_item_extent", (Opaque<LazyGridSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.item_extent = if v > 0.0 { Some(v) } else { None })
    });
    // lazy_grid_spacing — the cross/main cell gaps (the boa
    // `crossAxisSpacing` / `mainAxisSpacing` pair).
    rut_vm::pkg_fn!(pkg, "lazy_grid_spacing", (Opaque<LazyGridSpec>, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, cross: f64, main: f64| {
        b.with_mut(vm, |_vm, s| {
            s.cross_spacing = if cross > 0.0 { Some(cross) } else { None };
            s.main_spacing = if main > 0.0 { Some(main) } else { None };
        })
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_qkey", (Opaque<LazyGridSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "lazy_grid_build", (Opaque<LazyGridSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<LazyGridSpec>| {
        let view = b.with(|s| {
            Rc::new(LazyGridView::new_rut(
                s.builder.clone().expect("lazy_grid_build: no item builder"),
                s.count.clone().expect("lazy_grid_build: no count atom"),
                s.axis,
                Some(s.overscan.unwrap_or(0)),
                s.max_cross.expect("lazy_grid_build: no max cross extent"),
                s.aspect,
                s.item_extent,
                s.cross_spacing,
                s.main_spacing,
                s.query_key.clone(),
            )) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}
