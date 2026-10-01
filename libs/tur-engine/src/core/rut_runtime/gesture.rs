//! C4 — gestures, keyboard, and focus: the full `PointerInteract` surface
//! (down / move / up / context-menu beside click) and the `Focusable`
//! widget with key events, all reporting through the intent-queue rail.
//!
//! Payloads beyond the legacy `(u64, u64, f64)` click shape cross as
//! intent records ([`Intent::Pointer`] / [`Intent::Key`]) — the drain
//! dispatches each shape into its `entry fn` signature:
//!
//! - gesture callbacks: `entry fn cb(id: u64, lx: f64, ly: f64, gx: f64,
//!   gy: f64, button: u64)` (`button` is 0 for click / down-move-up
//!   gestures, 2 for the context-menu's right button)
//! - key callback: `entry fn cb(id: u64, key: str, code: str, mods: u64,
//!   kind: u64)` — `mods` bit0 shift / bit1 ctrl / bit2 alt / bit3 meta,
//!   `kind` 0 = down, 1 = up
//! - focus/blur callbacks: `entry fn cb(id: u64, _b: u64, _n: f64)`

use std::rc::Rc;

use crate::builtin_plugins::focus::FocusableView;
use crate::builtin_plugins::gesture::PointerInteractView;
use crate::core::edgy::mutation::MutationHandle;
use crate::core::edgy::value::Value;
use crate::core::focus::{BlurEvent, FocusEvent};
use crate::core::layout::HitTestBehavior;
use crate::core::platform::key_event::{KeydownEvent, KeyupEvent};
use rut_vm::Opaque;

use super::{Intent, RutHandles, RutView};

/// Declare the C4 rows on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        (
            "el_pi",
            vec![
                TY_U64,    // id_a (the callback's first argument)
                TY_U64,    // id_b (the callback's second argument)
                TY_STR,    // on_click ("" = absent)
                TY_STR,    // on_pointer_down ("" = absent)
                TY_U64,    // behavior const (0 opaque / 1 translucent)
                TY_STR,    // query key ("" = none)
                TY_OPAQUE, // child
            ],
            TY_OPAQUE,
        ),
        (
            "el_gesture",
            vec![
                TY_U64,    // id (the callback's first argument)
                TY_STR,    // on_click ("" = absent)
                TY_STR,    // on_pointer_down
                TY_STR,    // on_pointer_move
                TY_STR,    // on_pointer_up
                TY_STR,    // on_context_menu
                TY_OPAQUE, // child
            ],
            TY_OPAQUE,
        ),
        (
            "el_pi_empty",
            vec![
                TY_U64,    // id_a
                TY_U64,    // id_b
                TY_STR,    // on_click ("" = absent)
                TY_STR,    // on_pointer_down ("" = absent)
                TY_U64,    // behavior const
                TY_STR,    // query key ("" = none)
            ],
            TY_OPAQUE,
        ),
        (
            "el_gesture2",
            vec![
                TY_U64,    // id_a (the callback's first argument)
                TY_U64,    // id_b (the callback's second argument)
                TY_STR,    // on_click ("" = absent)
                TY_STR,    // on_pointer_down
                TY_STR,    // on_pointer_move
                TY_STR,    // on_pointer_up
                TY_STR,    // on_context_menu
                TY_OPAQUE, // child
            ],
            TY_OPAQUE,
        ),
        (
            "el_focusable",
            vec![
                TY_U64,    // id
                TY_STR,    // on_key_down ("" = absent)
                TY_STR,    // on_focus
                TY_STR,    // on_blur
                TY_OPAQUE, // child
            ],
            TY_OPAQUE,
        ),
        ("focus_request", vec![TY_U64], TY_NIL),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// Queue a pointer-intent mutation for `cb` (empty name = absent). The
/// mutation receives the pointer payload through the native crossing
/// (`[local.x, local.y, global.x, global.y]`).
fn pointer_mutation(
    handles: &Rc<RutHandles>,
    id: u64,
    cb: &str,
) -> Option<MutationHandle<crate::builtin_plugins::gesture::PointerInteractEvent>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, args| {
        // args[0] is the PointerInteractEvent payload (local.x, local.y,
        // global.x, global.y packed in order by the gesture bridge).
        let nums = |i: usize| match args.get(i) {
            Some(Value::Num(n)) => *n,
            _ => 0.0,
        };
        let (lx, ly, gx, gy) = (nums(0), nums(1), nums(2), nums(3));
        h.pending_calls.borrow_mut().push(Intent::Pointer {
            name: name.clone(),
            id,
            lx,
            ly,
            gx,
            gy,
            button: 0,
        });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// Queue a click intent (the `el_button` shape): `(name, id_a, id_b, seq)`.
fn click_mutation(
    handles: &Rc<RutHandles>,
    id_a: u64,
    id_b: u64,
    cb: &str,
) -> Option<MutationHandle<crate::builtin_plugins::gesture::PointerInteractEvent>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
        let n = h.click_seq.get() + 1;
        h.click_seq.set(n);
        h.pending_calls
            .borrow_mut()
            .push(Intent::Click { name: name.clone(), a: id_a, b: id_b, seq: n as f64 });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// Queue a context-menu intent (right button).
fn context_menu_mutation(
    handles: &Rc<RutHandles>,
    id: u64,
    cb: &str,
) -> Option<MutationHandle<crate::builtin_plugins::gesture::PointerInteractEvent>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
        h.pending_calls.borrow_mut().push(Intent::Pointer {
            name: name.clone(),
            id,
            lx: 0.0,
            ly: 0.0,
            gx: 0.0,
            gy: 0.0,
            button: 2,
        });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// Queue a key intent for `cb` (empty name = absent). The mutation
/// receives the keydown payload through the native crossing
/// (`[key, code, mods, kind]`).
fn key_mutation(
    handles: &Rc<RutHandles>,
    id: u64,
    cb: &str,
) -> Option<MutationHandle<KeydownEvent>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, args| {
        let arg_str = |i: usize| match args.get(i) {
            Some(Value::Str(s)) => s.to_string(),
            _ => String::new(),
        };
        let arg_num = |i: usize| match args.get(i) {
            Some(Value::Num(n)) => *n as u64,
            _ => 0,
        };
        h.pending_calls.borrow_mut().push(Intent::Key {
            name: name.clone(),
            id,
            key: arg_str(0),
            code: arg_str(1),
            modifiers: arg_num(2),
            kind: arg_num(3),
        });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// A focus/blur intent (no payload — the id is baked into the closure).
fn focus_mutation(
    handles: &Rc<RutHandles>,
    id: u64,
    cb: &str,
) -> Option<MutationHandle<FocusEvent>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
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

fn blur_mutation(
    handles: &Rc<RutHandles>,
    id: u64,
    cb: &str,
) -> Option<MutationHandle<BlurEvent>> {
    let name = cb.trim();
    if name.is_empty() {
        return None;
    }
    let name = name.to_string();
    let h = handles.clone();
    let dirty = handles.dirty.clone();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
        h.pending_calls.borrow_mut().push(Intent::Click {
            name: name.clone(),
            a: id,
            b: 1,
            seq: 1.0,
        });
        dirty.set(true);
        Ok(Value::Nil)
    });
    Some(MutationHandle::new(mutation))
}

/// Install the C4 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // A plain gesture pad with click + pointer-down intents, an explicit
    // hit-test behavior const (0 = Opaque default, 1 = Translucent) and a
    // query key ("" = none) — the corpus's general PointerInteract row.
    // Both ids cross to the entry (the `el_button` (id_a, id_b) shape).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_pi", (u64, u64, &str, &str, u64, &str, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id_a: u64, id_b: u64, click: &str, down: &str, behavior: u64, key: &str, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(PointerInteractView {
            behavior: Some(crate::core::view::Val::Static(match behavior {
                1 => HitTestBehavior::Translucent,
                _ => HitTestBehavior::Opaque,
            })),
            on_click: click_mutation(&h, id_a, id_b, click),
            on_pointer_down: pointer_mutation(&h, id_a, down),
            on_pointer_move: None,
            on_pointer_up: None,
            on_context_menu: None,
            query_key: if key.is_empty() {
                None
            } else {
                Some(vec![key.to_string()])
            },
            child: Some(child),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_gesture", (u64, &str, &str, &str, &str, &str, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64, click: &str, down: &str, mv: &str, up: &str, ctx: &str, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(PointerInteractView {
            behavior: Some(crate::core::view::Val::Static(HitTestBehavior::Opaque)),
            on_click: pointer_mutation(&h, id, click),
            on_pointer_down: pointer_mutation(&h, id, down),
            on_pointer_move: pointer_mutation(&h, id, mv),
            on_pointer_up: pointer_mutation(&h, id, up),
            on_context_menu: context_menu_mutation(&h, id, ctx),
            query_key: Some(vec!["rut".to_string(), "gesture".to_string()]),
            child: Some(child),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // The childless pad (the JS `PointerInteract().build()` twin — the
    // invisible hit-target widget).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_pi_empty", (u64, u64, &str, &str, u64, &str) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id_a: u64, id_b: u64, click: &str, down: &str, behavior: u64, key: &str| {
        let view = Rc::new(PointerInteractView {
            behavior: Some(crate::core::view::Val::Static(match behavior {
                1 => HitTestBehavior::Translucent,
                _ => HitTestBehavior::Opaque,
            })),
            on_click: click_mutation(&h, id_a, id_b, click),
            on_pointer_down: pointer_mutation(&h, id_a, down),
            on_pointer_move: None,
            on_pointer_up: None,
            on_context_menu: None,
            query_key: if key.is_empty() {
                None
            } else {
                Some(vec![key.to_string()])
            },
            child: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // Two-id gesture pad: both ids cross the intent (stateful pads keep
    // their display atoms in the entry args).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_gesture2", (u64, u64, &str, &str, &str, &str, &str, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, a: u64, b: u64, click: &str, down: &str, mv: &str, up: &str, ctx: &str, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let h2 = h.clone();
        let mk_pointer2 = |name: &str, kind: u8| {
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let name = name.to_string();
            let h3 = h2.clone();
            let dirty = h2.dirty.clone();
            let mutation = h2.store.bridge().build_mutate(move |_bridge, args| {
                let nums = |i: usize| match args.get(i) {
                    Some(Value::Num(n)) => *n,
                    _ => 0.0,
                };
                h3.pending_calls.borrow_mut().push(Intent::Pointer2 {
                    name: name.clone(),
                    a,
                    b,
                    lx: nums(0),
                    ly: nums(1),
                    gx: nums(2),
                    gy: nums(3),
                    button: kind as u64,
                });
                dirty.set(true);
                Ok(Value::Nil)
            });
            Some(MutationHandle::new(mutation))
        };
        let click = pointer_mutation(&h, a, click);
        let view = Rc::new(PointerInteractView {
            behavior: Some(crate::core::view::Val::Static(HitTestBehavior::Opaque)),
            on_click: click,
            on_pointer_down: mk_pointer2(down, 0),
            on_pointer_move: mk_pointer2(mv, 0),
            on_pointer_up: mk_pointer2(up, 0),
            on_context_menu: context_menu_mutation(&h, a, ctx),
            query_key: Some(vec!["rut".to_string(), "gesture".to_string()]),
            child: Some(child),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_focusable", (u64, &str, &str, &str, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64, key: &str, focus: &str, blur: &str, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(FocusableView {
            on_key_down: key_mutation(&h, id, key),
            on_key_up: None::<MutationHandle<KeyupEvent>>,
            on_focus: focus_mutation(&h, id, focus),
            on_blur: blur_mutation(&h, id, blur),
            child: Some(child),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // Programmatic focus: the FocusChange flush resolves the on_focus /
    // on_blur mutations next frame (the `requestFocus` bridge's twin).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "focus_request", (u64,) -> (), move |_vm: &mut rut_vm::interp::Vm, node: u64| {
        h.focus_manager
            .borrow_mut()
            .set_focus(crate::core::element::ElementNodeId::new(node));
        h.dirty.set(true);
        Ok(())
    });
}
