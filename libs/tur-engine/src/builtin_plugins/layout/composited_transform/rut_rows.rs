//! The composited-transform families' `tur_host` pkg rows (via the
//! pkg-extension seam): `ct_link_new` mints a `LayerLink` (registered into
//! the subsystem's registry, exactly like the JS `createLayerLink`
//! factory), and the CompositedTransformTarget / CompositedTransformFollower
//! spec families.
//!
//! Anchors cross as `u64` alignment consts (the `ALIGN_*` values — the
//! `Alignment` `FromPrimitive` order); the follower's `targetOffset` is a
//! plain `Value` map (`{x, y}`) built host-side from two f64s.

use std::rc::Rc;

use crate::builtin_plugins::layout::composited_transform::follower::FollowerView;
use crate::builtin_plugins::layout::composited_transform::link::CompositedLinkState;
use crate::builtin_plugins::layout::composited_transform::target::TargetView;
use crate::builtin_plugins::layout::LayerLinkRegistry;
use crate::core::edgy::value::Value;
use crate::core::layout::Alignment;
use crate::core::rut_runtime::{RutHandles, RutView};
use crate::core::view::Val;

use rut_vm::Opaque;

use crate::builtin_plugins::layout::rut_rows::{RutLayerLink, alignment_of};

/// The pkg-extension payload: decl rows at compile time, bodies at boot.
pub(crate) fn install_decl_ext(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    install_decl(cx);
    let Some(handles) = cx.handles else {
        return; // the compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    install(&mut *cx.pkg, &handles);
}

/// Declare the composited rows (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        row("ct_link_new", vec![], TY_OPAQUE),
        // target
        row("ct_target_new", vec![], TY_OPAQUE),
        row("ct_target_link", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("ct_target_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("ct_target_build", vec![TY_OPAQUE], TY_OPAQUE),
        // follower
        row("ct_follower_new", vec![], TY_OPAQUE),
        row("ct_follower_link", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("ct_follower_target_anchor", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("ct_follower_target_anchor_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("ct_follower_follower_anchor", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("ct_follower_offset", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        row("ct_follower_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("ct_follower_build", vec![TY_OPAQUE], TY_OPAQUE),
    ]);
}

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_U64};

/// The composited-target spec.
pub(crate) struct CtTargetSpec {
    link: Option<Rc<CompositedLinkState>>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

/// The composited-follower spec.
pub(crate) struct CtFollowerSpec {
    link: Option<Rc<CompositedLinkState>>,
    target_anchor: Option<Val<Alignment>>,
    follower_anchor: Option<Val<Alignment>>,
    offset: Option<(f64, f64)>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

/// Install the composited-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // Mint + register a link (the JS factory's twin — the subsystem
    // recomputes only registered links).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ct_link_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let registry = h
            .inst
            .plugin_state::<LayerLinkRegistry>()
            .ok_or_else(|| {
                rut_vm::Trap::new(
                    rut_vm::TrapKind::Invalid,
                    "ct_link_new: composited-transform not registered on this instance",
                )
            })?;
        let state = Rc::new(CompositedLinkState::default());
        registry.0.borrow_mut().push(state.clone());
        Ok(Opaque::alloc(vm, RutLayerLink(state))?.handle().clone())
    });

    // ---- target -------------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "ct_target_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, CtTargetSpec { link: None, child: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "ct_target_link", (Opaque<CtTargetSpec>, Opaque<RutLayerLink>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtTargetSpec>, link: Opaque<RutLayerLink>| {
        let link = link.with(|l| l.0.clone())?;
        b.with_mut(vm, |_vm, s| s.link = Some(link))
    });
    rut_vm::pkg_fn!(pkg, "ct_target_child", (Opaque<CtTargetSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtTargetSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "ct_target_build", (Opaque<CtTargetSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<CtTargetSpec>| {
        let view = b.with(|s| {
            Rc::new(TargetView::new_rut(s.link.clone(), s.child.clone())) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- follower -----------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "ct_follower_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, CtFollowerSpec { link: None, target_anchor: None, follower_anchor: None, offset: None, child: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "ct_follower_link", (Opaque<CtFollowerSpec>, Opaque<RutLayerLink>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>, link: Opaque<RutLayerLink>| {
        let link = link.with(|l| l.0.clone())?;
        b.with_mut(vm, |_vm, s| s.link = Some(link))
    });
    rut_vm::pkg_fn!(pkg, "ct_follower_target_anchor", (Opaque<CtFollowerSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>, a: u64| {
        b.with_mut(vm, |_vm, s| s.target_anchor = Some(Val::Static(alignment_of(a))))
    });
    // Reactive-anchor twin: the target anchor resolves from an atom (the
    // atom holds the Alignment enum code; a button flips it).
    rut_vm::pkg_fn!(pkg, "ct_follower_target_anchor_bound", (Opaque<CtFollowerSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| {
            s.target_anchor = Some(Val::Reactive(crate::core::edgy::reactive::Readable::Source(
                crate::core::edgy::reactive::Source::<Alignment>::from_id(
                    crate::core::edgy::reactive::AtomId(atom as u32),
                ),
            )));
        })
    });
    rut_vm::pkg_fn!(pkg, "ct_follower_follower_anchor", (Opaque<CtFollowerSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>, a: u64| {
        b.with_mut(vm, |_vm, s| s.follower_anchor = Some(Val::Static(alignment_of(a))))
    });
    rut_vm::pkg_fn!(pkg, "ct_follower_offset", (Opaque<CtFollowerSpec>, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>, ox: f64, oy: f64| {
        b.with_mut(vm, |_vm, s| s.offset = Some((ox, oy)))
    });
    rut_vm::pkg_fn!(pkg, "ct_follower_child", (Opaque<CtFollowerSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "ct_follower_build", (Opaque<CtFollowerSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<CtFollowerSpec>| {
        let view = b.with(|s| {
            let target_anchor = s.target_anchor.clone().expect("ct_follower_build: no target anchor");
            let follower_anchor = s.follower_anchor.clone().expect("ct_follower_build: no follower anchor");
            let target_offset = s.offset.map(|(ox, oy)| {
                Val::Static(Value::map([
                    ("x", Value::Num(ox)),
                    ("y", Value::Num(oy)),
                ]))
            });
            Rc::new(FollowerView::new_rut(
                s.link.clone(),
                target_anchor,
                follower_anchor,
                target_offset,
                false,
                s.child.clone(),
            )) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}
