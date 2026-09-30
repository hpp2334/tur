//! Phase-4 corpus rows — composited transforms: `ct_link_new` mints a
//! `LayerLink` (registered into the subsystem's registry, exactly like the
//! JS `createLayerLink` factory), `el_ct_target` / `el_ct_follower` wrap
//! children.
//!
//! Anchors cross as `u64` alignment consts (the `ALIGN_*` values — the
//! `Alignment` `FromPrimitive` order); the follower's `targetOffset` is a
//! plain `Value` map (`{x, y}`) built host-side from two f64s.

use std::rc::Rc;

use crate::builtin_plugins::layout::FollowerView;
use crate::builtin_plugins::layout::TargetView;
use crate::builtin_plugins::layout::composited_transform::link::CompositedLinkState;
use crate::builtin_plugins::layout::LayerLinkRegistry;
use crate::core::edgy::value::Value;
use crate::core::layout::Alignment;
use crate::core::view::Val;
use num_traits::FromPrimitive;

use rut_vm::Opaque;

use super::{RutView, RutHandles};

pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        ("ct_link_new", vec![], TY_OPAQUE),
        ("el_ct_target", vec![TY_OPAQUE, TY_OPAQUE], TY_OPAQUE),
        (
            "el_ct_follower",
            vec![TY_OPAQUE, TY_U64, TY_U64, TY_F64, TY_F64, TY_OPAQUE],
            TY_OPAQUE,
        ),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// A `LayerLink` opaque — the shared `CompositedLinkState` Rc.
pub(crate) struct RutLayerLink(pub Rc<CompositedLinkState>);

pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // Mint + register a link (the JS factory's twin — the subsystem
    // recomputes only registered links).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ct_link_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let registry = h
            .js_ctx
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

    rut_vm::pkg_fn!(pkg, "el_ct_target", (Opaque<RutLayerLink>, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, link: Opaque<RutLayerLink>, child: Opaque<RutView>| {
        let link = link.with(|l| l.0.clone())?;
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(TargetView::new_rut(Some(link), Some(child)));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // el_ct_follower(link, target_anchor, follower_anchor, off_x, off_y, child).
    rut_vm::pkg_fn!(pkg, "el_ct_follower", (Opaque<RutLayerLink>, u64, u64, f64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, link: Opaque<RutLayerLink>, ta: u64, fa: u64, ox: f64, oy: f64, child: Opaque<RutView>| {
        let link = link.with(|l| l.0.clone())?;
        let child = child.with(|v| v.0.clone())?;
        let target_offset = (ox != 0.0 || oy != 0.0).then(|| {
            Val::Static(Value::map([
                ("x", Value::Num(ox)),
                ("y", Value::Num(oy)),
            ]))
        });
        let view = Rc::new(FollowerView::new_rut(
            Some(link),
            Val::Static(anchor_of(ta)),
            Val::Static(anchor_of(fa)),
            target_offset,
            false,
            Some(child),
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}

fn anchor_of(v: u64) -> Alignment {
    Alignment::from_u64(v).unwrap_or(Alignment::TopLeft)
}
