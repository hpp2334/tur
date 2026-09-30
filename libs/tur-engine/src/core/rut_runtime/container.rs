//! C3 — the Container full surface: border / radius / shadow / alignment /
//! size / clip, plus `SizedBox`, authored through a builder opaque with
//! setter rows. Flags cross as `u64` consts (the decl module exports the
//! `ALIGN_*` / `CLIP_*` / `BORDER_*` values — rut enums do not cross).

use std::rc::Rc;

use crate::builtin_plugins::layout::ContainerView;
use crate::core::layout::{Alignment, BorderPosition, ClipBehavior};
use num_traits::FromPrimitive;
use crate::core::render::brush::Brush;
use crate::core::view::Val;

use rut_vm::Opaque;

use super::color_of;
use super::{RutView, ViewBuilder};

/// Declare the C3 rows + flag consts on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        ("el_box_new", vec![], TY_OPAQUE),
        ("box_padding", vec![TY_OPAQUE, TY_F64], TY_NIL),
        ("box_color", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("box_size", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        (
            "box_border",
            vec![TY_OPAQUE, TY_U64, TY_F64, TY_U64],
            TY_NIL,
        ),
        ("box_radius", vec![TY_OPAQUE, TY_F64], TY_NIL),
        (
            "box_shadow",
            vec![TY_OPAQUE, TY_U64, TY_F64, TY_F64, TY_F64],
            TY_NIL,
        ),
        ("box_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("box_clip", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("el_sizedbox", vec![TY_F64, TY_F64, TY_OPAQUE], TY_OPAQUE),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// The flag consts (u64) exported by the `tur` decl module — the crossing
/// stand-ins for the engine enums (values are the `FromPrimitive` order).
pub fn decl_consts() -> Vec<(String, rut_core::types::TypeId, u64)> {
    use rut_core::types::TY_U64;
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    vec![
        c("ALIGN_TOP_LEFT", 0),
        c("ALIGN_TOP_CENTER", 1),
        c("ALIGN_TOP_RIGHT", 2),
        c("ALIGN_CENTER_LEFT", 3),
        c("ALIGN_CENTER", 4),
        c("ALIGN_CENTER_RIGHT", 5),
        c("ALIGN_BOTTOM_LEFT", 6),
        c("ALIGN_BOTTOM_CENTER", 7),
        c("ALIGN_BOTTOM_RIGHT", 8),
        c("CLIP_NONE", 0),
        c("CLIP_HARD_EDGE", 1),
        c("CLIP_ANTI_ALIAS", 2),
        c("BORDER_INSIDE", 0),
        c("BORDER_CENTER", 1),
        c("BORDER_OUTSIDE", 2),
    ]
}

pub(crate) fn alignment_of(v: u64) -> Alignment {
    Alignment::from_u64(v).unwrap_or(Alignment::Center)
}

fn clip_of(v: u64) -> ClipBehavior {
    ClipBehavior::from_u64(v).unwrap_or(ClipBehavior::None)
}

fn border_position_of(v: u64) -> BorderPosition {
    BorderPosition::from_u64(v).unwrap_or(BorderPosition::Inside)
}

/// Install the C3 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, _handles: &std::rc::Rc<super::RutHandles>) {
    rut_vm::pkg_fn!(pkg, "el_box_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(rut_vm::Opaque::alloc(vm, ViewBuilder::Box(Box::<ContainerView>::default()))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "box_padding", (Opaque<super::ViewBuilder>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, v: f64| {
        with_box(b, vm, |box_| box_.padding = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "box_color", (Opaque<super::ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, color: u64| {
        with_box(b, vm, |box_| box_.color = Some(Val::Static(Brush::SolidColor(color_of(color)))))
    });
    rut_vm::pkg_fn!(pkg, "box_size", (Opaque<super::ViewBuilder>, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, w: f64, h: f64| {
        with_box(b, vm, |box_| {
            box_.width = if w > 0.0 { Some(Val::Static(w)) } else { None };
            box_.height = if h > 0.0 { Some(Val::Static(h)) } else { None };
        })
    });
    rut_vm::pkg_fn!(pkg, "box_border", (Opaque<super::ViewBuilder>, u64, f64, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, color: u64, width: f64, position: u64| {
        with_box(b, vm, |box_| {
            box_.border_color = Some(Val::Static(color_of(color)));
            box_.border_width = Some(Val::Static(width));
            box_.border_position = Some(Val::Static(border_position_of(position)));
        })
    });
    rut_vm::pkg_fn!(pkg, "box_radius", (Opaque<super::ViewBuilder>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, r: f64| {
        with_box(b, vm, |box_| box_.border_radius = Some(Val::Static(r)))
    });
    rut_vm::pkg_fn!(pkg, "box_shadow", (Opaque<super::ViewBuilder>, u64, f64, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, color: u64, blur: f64, dx: f64, dy: f64| {
        with_box(b, vm, |box_| {
            box_.shadow_color = Some(Val::Static(color_of(color)));
            box_.shadow_blur = Some(Val::Static(blur));
            box_.shadow_offset = Some((dx, dy));
        })
    });
    rut_vm::pkg_fn!(pkg, "box_align", (Opaque<super::ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, a: u64| {
        with_box(b, vm, |box_| box_.alignment = Some(Val::Static(alignment_of(a))))
    });
    rut_vm::pkg_fn!(pkg, "box_clip", (Opaque<super::ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, c: u64| {
        with_box(b, vm, |box_| box_.clip_behavior = Some(Val::Static(clip_of(c))))
    });
    // SizedBox: an exactly-sized wrapper (the JS `SizedBox(w, h)` twin — a
    // Container with only width/height set).
    rut_vm::pkg_fn!(pkg, "el_sizedbox", (f64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, w: f64, h: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(ContainerView {
            width: Some(Val::Static(w)),
            height: Some(Val::Static(h)),
            children: vec![child],
            ..ContainerView::default()
        });
        Ok(rut_vm::Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}

/// Mutate the `ContainerView` payload of a box builder.
fn with_box(
    b: Opaque<super::ViewBuilder>,
    vm: &mut rut_vm::interp::Vm,
    f: impl FnOnce(&mut ContainerView),
) -> Result<(), rut_vm::Trap> {
    b.with_mut(vm, |_vm, b| {
        match b {
            ViewBuilder::Box(box_) => f(box_),
            // A setter on a non-box builder is a wiring bug — trap loudly.
            _ => {
                return Err(rut_vm::Trap::new(
                    rut_vm::TrapKind::Invalid,
                    "box_* setter on a non-box builder",
                ))
            }
        }
        Ok(())
    })?
}
