//! C5 — animation rows on the `tur` rut host pkg (via the pkg-extension
//! seam): `Opacity` / `Transform` effect rows, the animation-controller
//! opaque (a Rust-held controller registered into the same
//! [`AnimationManager`] the subsystem ticks), the `onTick` entry rail, and
//! the tween helpers.
//!
//! The controller is NOT realm-minted: `AnimationManager` gained a
//! [`ControllerFace`](crate::manager::ControllerFace) (Js | Rust) so a rut
//! controller is an `Rc<RefCell<AnimationController>>` — realm-free mint,
//! ticking, and control. `onTick` / `onEnd` ride the engine's intent rail
//! (the native `AnimationTickEvent` crossing carries the eased progress;
//! the entry fn receives it as its second argument).

use std::rc::Rc;

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};
use rut_vm::Opaque;
use tur_engine::builtin_plugins::effects::{OpacityView, TransformView};
use tur_engine::core::rut_runtime::{Intent, RutHandles, RutView, readable_of};

use crate::controller::{AnimationController, RepeatMode};
use crate::curve::Curve;
use crate::manager::ControllerFace;
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
    cx.decl.extend(
        vec![
            ("el_opacity", vec![TY_F64, TY_OPAQUE], TY_OPAQUE),
            ("el_opacity_bound", vec![TY_U64, TY_OPAQUE], TY_OPAQUE),
            (
                "el_transform",
                vec![TY_F64, TY_F64, TY_F64, TY_F64, TY_OPAQUE],
                TY_OPAQUE,
            ),
            (
                "anim_ctrl",
                vec![TY_U64, TY_F64, TY_STR, TY_U64, TY_STR, TY_STR],
                TY_OPAQUE,
            ),
            ("anim_forward", vec![TY_OPAQUE], TY_NIL),
            ("anim_reverse", vec![TY_OPAQUE], TY_NIL),
            ("anim_pause", vec![TY_OPAQUE], TY_NIL),
            ("anim_resume", vec![TY_OPAQUE], TY_NIL),
            ("anim_stop", vec![TY_OPAQUE], TY_NIL),
            ("anim_seek", vec![TY_OPAQUE, TY_F64], TY_NIL),
            ("anim_value", vec![TY_OPAQUE], TY_F64),
            ("anim_status", vec![TY_OPAQUE], TY_STR),
            ("anim_repeat", vec![TY_OPAQUE, TY_U64], TY_NIL),
            ("tween_lerp", vec![TY_F64, TY_F64, TY_F64], TY_F64),
            ("color_tween_lerp", vec![TY_U64, TY_U64, TY_F64], TY_U64),
            ("curve_eval", vec![TY_STR, TY_F64], TY_F64),
        ]
        .into_iter()
        .map(|(n, p, r)| (n.to_string(), p, r)),
    );

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
            .register_controller(ControllerFace::Rust(ctrl.clone()));
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

    // ---- the controller opaque (Rust-held; the manager ticks it) --------
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "anim_ctrl", (u64, f64, &str, u64, &str, &str) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64, duration_ms: f64, curve: &str, repeat: u64, on_tick: &str, on_end: &str| {
        let _ = &h;
        let mut ctrl = AnimationController::new(duration_ms.max(1.0) as u64, curve_of(curve));
        ctrl.set_repeat_mode(repeat_of(repeat));
        // onTick / onEnd as intent mutations — the native crossing delivers
        // the eased progress; the entry fn gets (id, eased).
        let wire_tick = |name: &str, h: &Rc<RutHandles>| {
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let name = name.to_string();
            let h2 = h.clone();
            let dirty = h.dirty.clone();
            let mutation = h.store.bridge().build_mutate(move |_bridge, args, _boa| {
                let t = match args.first() {
                    Some(tur_engine::core::edgy::Value::Num(n)) => *n,
                    _ => 0.0,
                };
                h2.pending_calls.borrow_mut().push(Intent::Value {
                    name: name.clone(),
                    a: id,
                    value: tur_engine::core::edgy::Value::Num(t),
                });
                dirty.set(true);
                Ok(tur_engine::core::edgy::Value::Nil)
            });
            Some(tur_engine::core::edgy::mutation::MutationHandle::<
                AnimationTickEvent,
            >::new(mutation))
        };
        if let Some(m) = wire_tick(on_tick, &h) {
            ctrl.set_on_tick(m);
        }
        let wire_end = |name: &str, h: &Rc<RutHandles>| {
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let name = name.to_string();
            let h2 = h.clone();
            let dirty = h.dirty.clone();
            let mutation = h.store.bridge().build_mutate(move |_bridge, _args, _boa| {
                h2.pending_calls.borrow_mut().push(Intent::Value {
                    name: name.clone(),
                    a: id,
                    value: tur_engine::core::edgy::Value::Nil,
                });
                dirty.set(true);
                Ok(tur_engine::core::edgy::Value::Nil)
            });
            Some(tur_engine::core::edgy::mutation::MutationHandle::<
                AnimationEndEvent,
            >::new(mutation))
        };
        if let Some(m) = wire_end(on_end, &h) {
            ctrl.set_on_end(m);
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
        let now = h.clock.now().millis_since_epoch();
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().forward_at(now);
        reg(&rc);
        Ok(())
    });
    let h = handles.clone();
    let reg = register_into.clone();
    rut_vm::pkg_fn!(pkg, "anim_reverse", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now().millis_since_epoch();
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().reverse_at(now);
        reg(&rc);
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "anim_pause", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now().millis_since_epoch();
        let rc = c.with(|c| c.0.clone())?;
        rc.borrow_mut().pause_at(now);
        Ok(())
    });
    let h = handles.clone();
    let reg = register_into.clone();
    rut_vm::pkg_fn!(pkg, "anim_resume", (Opaque<RutAnimCtrl>,) -> (), move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutAnimCtrl>| {
        let now = h.clock.now().millis_since_epoch();
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
        let now = h.clock.now().millis_since_epoch();
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

    // ---- tween + curve helpers (pure math) ------------------------------
    rut_vm::pkg_fn!(pkg, "tween_lerp", (f64, f64, f64) -> f64, |_vm: &mut rut_vm::interp::Vm, begin: f64, end: f64, t: f64| {
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
