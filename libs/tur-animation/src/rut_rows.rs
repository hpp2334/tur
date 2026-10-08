//! C5 — animation rows on the `tur_host` rut pkg (via the pkg-extension
//! seam): `Opacity` / `Transform` effect rows, the animation-controller
//! opaque (a Rust-held controller registered into the same
//! [`AnimationManager`] the subsystem ticks), the `onTick` rail, and
//! the tween helpers.
//!
//! The controller is NOT realm-minted: `AnimationManager` gained a
//! [`ControllerFace`](crate::manager::ControllerFace) (Js | Rust) so a rut
//! controller is an `Rc<RefCell<AnimationController>>` — realm-free mint,
//! ticking, and control. `onTick` / `onEnd` are SEALED MUTATIONS (`mutate_f64`
//! ticks `(ctx, v: f64)`; `mutate` ends `(ctx)` — the controller enqueues
//! the invocation with the eased progress; the flush's mutation pass fires
//! it). 0 = absent.

use std::rc::Rc;

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};
use rut_vm::Opaque;
use tur_engine::builtin_plugins::effects::{OpacityView, TransformView};
use tur_engine::core::rut_runtime::{RutView, readable_of};

use crate::controller::{AnimationController, RepeatMode};
use crate::curve::Curve;
use crate::tween::{NumTween, Tween};
use crate::{event::AnimationEndEvent, event::AnimationTickEvent};

/// The Rust-held controller opaque.
pub struct RutAnimCtrl(pub Rc<std::cell::RefCell<AnimationController>>);

/// Declare the C5 rows + install the bodies — the
/// [`RutPkgExt`](tur_engine::core::rut_runtime::RutPkgExt) payload.
/// `manager` is the plugin's shared registry (captured at register time);
/// every minted rut controller registers into it.
pub fn install(
    cx: &mut tur_engine::core::rut_runtime::RutPkgCx<'_>,
    manager: Rc<std::cell::RefCell<crate::manager::AnimationManager>>,
) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        row("el_opacity", vec![TY_F64, TY_OPAQUE], TY_OPAQUE),
        row("el_opacity_bound", vec![TY_U64, TY_OPAQUE], TY_OPAQUE),
        row(
            "el_transform",
            vec![TY_F64, TY_F64, TY_F64, TY_F64, TY_OPAQUE],
            TY_OPAQUE,
        ),
        // The bound twins: one channel rides a live f64 atom (the
        // Val-backed bind machinery — `Val::Reactive` in the view field,
        // subscribed by the element, resolved by layout into `painting`);
        // the other channels stay static. Slot positions match `el_transform`
        // exactly — a bound channel crosses as a u64 atom id. A rotating
        // showcase never re-mounts: the atom swap re-resolves the angle
        // through the subscribe → relayout rail.
        row(
            "el_transform_angle_bound",
            vec![TY_F64, TY_U64, TY_F64, TY_F64, TY_OPAQUE],
            TY_OPAQUE,
        ),
        row(
            "el_transform_scale_bound",
            vec![TY_U64, TY_F64, TY_F64, TY_F64, TY_OPAQUE],
            TY_OPAQUE,
        ),
        row(
            "el_transform_translate_bound",
            vec![TY_F64, TY_F64, TY_U64, TY_U64, TY_OPAQUE],
            TY_OPAQUE,
        ),
        // The M2 mutation rail: the controller's tick/end are SEALED
        // MUTATIONS (`mutate_f64` ticks `(ctx, v: f64)`; `mutate` ends
        // `(ctx)`); 0 = absent. The four legacy cb-twin rows died with the
        // M2 corpus sweep.
        row(
            "anim_ctrl_mut",
            vec![TY_F64, TY_STR, TY_U64, TY_U64, TY_U64],
            TY_OPAQUE,
        ),
        row("anim_forward", vec![TY_OPAQUE], TY_NIL),
        row("anim_reverse", vec![TY_OPAQUE], TY_NIL),
        row("anim_pause", vec![TY_OPAQUE], TY_NIL),
        row("anim_resume", vec![TY_OPAQUE], TY_NIL),
        row("anim_stop", vec![TY_OPAQUE], TY_NIL),
        row("anim_seek", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("anim_value", vec![TY_OPAQUE], TY_F64),
        row("anim_status", vec![TY_OPAQUE], TY_STR),
        row("anim_repeat", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("anim_speed", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("tween_lerp", vec![TY_F64, TY_F64, TY_F64], TY_F64),
        row("color_tween_lerp", vec![TY_U64, TY_U64, TY_F64], TY_U64),
        // The kit wrappers' twin spellings (the `mount_raw` pattern — the
        // kit's own wrapper fns shadow the row names, so their bodies
        // reach the rows through these aliases).
        row("tween_lerp_raw", vec![TY_F64, TY_F64, TY_F64], TY_F64),
        row("color_tween_lerp_raw", vec![TY_U64, TY_U64, TY_F64], TY_U64),
        row("curve_eval", vec![TY_STR, TY_F64], TY_F64),
    ]);

    // The bodies need the per-instance handles — absent at the compile-time
    // decl probe.
    let Some(handles) = cx.handles else {
        return;
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;
    // Every `anim_forward` / `anim_reverse` / `anim_resume` row registers
    // into this manager (the subsystem's tick source).
    let manager_for_rows = manager.clone();
    let register_into = move |ctrl: &Rc<std::cell::RefCell<AnimationController>>| {
        manager_for_rows
            .borrow_mut()
            .register_controller(ctrl.clone());
    };

    // ---- effect rows -----------------------------------------------------
    rut_vm::pkg_fn!(pkg, "el_opacity", (f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, value: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(OpacityView::new_rut(
            Some(tur_engine::core::view::Val::Static(value as f32)),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // An opacity bound to an f64 atom — the onTick rail's consumer.
    rut_vm::pkg_fn!(pkg, "el_opacity_bound", (u64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, atom: u64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(OpacityView::new_rut(
            Some(tur_engine::core::view::Val::Reactive(readable_of::<f32>(atom))),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_transform", (f64, f64, f64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, scale: f64, rotate: f64, tx: f64, ty: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(TransformView::new_rut(
            Some(tur_engine::core::view::Val::Static(scale)),
            Some(tur_engine::core::view::Val::Static(rotate)),
            Some(tur_engine::core::view::Val::Static(tx)),
            Some(tur_engine::core::view::Val::Static(ty)),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // The bound twins (`Val::Reactive` on the bound channel — the
    // `el_opacity_bound` law): the element subscribes the atom, layout
    // re-resolves `painting`, and the subtree never re-mounts.
    rut_vm::pkg_fn!(pkg, "el_transform_angle_bound", (f64, u64, f64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, scale: f64, rotate_atom: u64, tx: f64, ty: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(TransformView::new_rut(
            Some(tur_engine::core::view::Val::Static(scale)),
            Some(tur_engine::core::view::Val::Reactive(readable_of::<f64>(rotate_atom))),
            Some(tur_engine::core::view::Val::Static(tx)),
            Some(tur_engine::core::view::Val::Static(ty)),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_transform_scale_bound", (u64, f64, f64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, scale_atom: u64, rotate: f64, tx: f64, ty: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(TransformView::new_rut(
            Some(tur_engine::core::view::Val::Reactive(readable_of::<f64>(scale_atom))),
            Some(tur_engine::core::view::Val::Static(rotate)),
            Some(tur_engine::core::view::Val::Static(tx)),
            Some(tur_engine::core::view::Val::Static(ty)),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_transform_translate_bound", (f64, f64, u64, u64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, scale: f64, rotate: f64, tx_atom: u64, ty_atom: u64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(TransformView::new_rut(
            Some(tur_engine::core::view::Val::Static(scale)),
            Some(tur_engine::core::view::Val::Static(rotate)),
            Some(tur_engine::core::view::Val::Reactive(readable_of::<f64>(tx_atom))),
            Some(tur_engine::core::view::Val::Reactive(readable_of::<f64>(ty_atom))),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- the controller opaque (Rust-held; the manager ticks it) --------
    // One arity over one mint: the tick/end are sealed mutations (0 =
    // absent). The subsystem enqueues the tick invocation with the eased
    // progress; the flush's mutation pass invokes the kit-sealed closure
    // with the ctx (the tick fn's `(ctx, v: f64)` crossing).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "anim_ctrl_mut", (f64, &str, u64, u64, u64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, duration_ms: f64, curve: &str, repeat: u64, on_tick: u64, on_end: u64| {
        let mut ctrl = AnimationController::new(duration_ms.max(1.0) as u64, curve_of(curve));
        ctrl.set_repeat_mode(repeat_of(repeat));
        if on_tick != 0 {
            ctrl.set_on_tick(tur_engine::core::edgy::mutation::MutationHandle::<
                AnimationTickEvent,
            >::new(tur_engine::core::rut_runtime::mutation_of(on_tick)));
        }
        if on_end != 0 {
            ctrl.set_on_end(tur_engine::core::edgy::mutation::MutationHandle::<
                AnimationEndEvent,
            >::new(tur_engine::core::rut_runtime::mutation_of(on_end)));
        }
        ctrl.set_mutation_queue(h.mutation_queue.clone());
        let rc: Rc<std::cell::RefCell<AnimationController>> =
            Rc::new(std::cell::RefCell::new(ctrl));
        Ok(Opaque::alloc(vm, RutAnimCtrl(rc))?.handle().clone())
    });

    // ---- control rows (forward/reverse/resume re-register into the
    //      manager; the RefCell borrow never spans the register) ----------
    let h = handles.clone();
    let reg = register_into.clone();
    rut_vm::pkg_fn!(pkg, "anim_forward", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now_millis() as u64;
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().forward_at(now);
        reg(&rc);
        Ok(())
    });
    let h = handles.clone();
    let reg = register_into.clone();
    rut_vm::pkg_fn!(pkg, "anim_reverse", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now_millis() as u64;
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().reverse_at(now);
        reg(&rc);
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "anim_pause", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now_millis() as u64;
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().pause_at(now);
        Ok(())
    });
    let h = handles.clone();
    let reg = register_into.clone();
    rut_vm::pkg_fn!(pkg, "anim_resume", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now_millis() as u64;
        let rc = c.with(|c| c.0.clone())?;
        let active = {
            let mut ctrl = rc.borrow_mut();
            ctrl.resume_at(now);
            ctrl.is_active()
        };
        if active {
            reg(&rc);
        }
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "anim_stop", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().stop_now();
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "anim_seek", (Opaque<RutAnimCtrl>, f64) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>, t: f64| {
        let now = h.clock.now_millis() as u64;
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().seek_to(t, now);
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "anim_value", (Opaque<RutAnimCtrl>,) -> f64, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        c.with(|c| Ok(c.0.borrow().value()))?
    });
    rut_vm::pkg_fn!(pkg, "anim_status", (Opaque<RutAnimCtrl>,) -> String, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        c.with(|c| Ok(c.0.borrow().status_name().to_string()))?
    });
    rut_vm::pkg_fn!(pkg, "anim_repeat", (Opaque<RutAnimCtrl>, u64) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>, repeat: u64| {
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().set_repeat_mode(repeat_of(repeat));
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "anim_speed", (Opaque<RutAnimCtrl>, f64) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>, speed: f64| {
        let now = h.clock.now_millis() as u64;
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().set_speed_to(speed, now);
        Ok(())
    });

    // ---- tween + curve helpers (pure math) ------------------------------
    rut_vm::pkg_fn!(pkg, "tween_lerp", (f64, f64, f64) -> f64, |_vm: &mut rut_vm::interp::Vm, begin: f64, end: f64, t: f64| {
        Ok(NumTween::new(begin, end).lerp(t))
    });
    // The kit wrappers' twin spellings — SAME bodies (see the decl note).
    rut_vm::pkg_fn!(pkg, "tween_lerp_raw", (f64, f64, f64) -> f64, |_vm: &mut rut_vm::interp::Vm, begin: f64, end: f64, t: f64| {
        Ok(NumTween::new(begin, end).lerp(t))
    });
    // Component-wise u8 lerp — the same math as `Color::lerp` (the
    // `ColorTween` payload), over the packed 0xRRGGBBAA crossing form.
    rut_vm::pkg_fn!(pkg, "color_tween_lerp", (u64, u64, f64) -> u64, |_vm: &mut rut_vm::interp::Vm, begin: u64, end: u64, t: f64| {
        let t = t.clamp(0.0, 1.0);
        let ch = |x: u64, y: u64| -> u64 {
            if t == 0.0 {
                x
            } else if t == 1.0 {
                y
            } else {
                let v = (x & 0xFF) as f64 + (((y & 0xFF) as f64) - ((x & 0xFF) as f64)) * t;
                v.round().clamp(0.0, 255.0) as u64
            }
        };
        let mix = |x: u64, y: u64| -> u64 {
            (ch(x >> 24, y >> 24) << 24)
                | (ch(x >> 16, y >> 16) << 16)
                | (ch(x >> 8, y >> 8) << 8)
                | ch(x, y)
        };
        Ok(mix(begin, end))
    });
    // The kit wrappers' twin spelling — SAME body (see the decl note).
    rut_vm::pkg_fn!(pkg, "color_tween_lerp_raw", (u64, u64, f64) -> u64, |_vm: &mut rut_vm::interp::Vm, begin: u64, end: u64, t: f64| {
        let t = t.clamp(0.0, 1.0);
        let ch = |x: u64, y: u64| -> u64 {
            if t == 0.0 {
                x
            } else if t == 1.0 {
                y
            } else {
                let v = (x & 0xFF) as f64 + (((y & 0xFF) as f64) - ((x & 0xFF) as f64)) * t;
                v.round().clamp(0.0, 255.0) as u64
            }
        };
        let mix = |x: u64, y: u64| -> u64 {
            (ch(x >> 24, y >> 24) << 24)
                | (ch(x >> 16, y >> 16) << 16)
                | (ch(x >> 8, y >> 8) << 8)
                | ch(x, y)
        };
        Ok(mix(begin, end))
    });
    rut_vm::pkg_fn!(pkg, "curve_eval", (&str, f64) -> f64, |_vm: &mut rut_vm::interp::Vm, curve: &str, t: f64| {
        Ok(curve_of(curve).transform(t))
    });
}

/// Parse a curve name (the `Curve::from_str` vocabulary; "" = Linear).
fn curve_of(name: &str) -> Curve {
    name.parse().unwrap_or(Curve::Linear)
}

/// `0` = none, `u64::MAX` = infinite, `n` = n plays (the `repeat` flag).
fn repeat_of(repeat: u64) -> RepeatMode {
    match repeat {
        0 => RepeatMode::default(),
        u64::MAX => RepeatMode::Infinite,
        n => RepeatMode::Finite(n),
    }
}
