//! The gesture families' `tur_host` pkg rows (via the pkg-extension seam):
//! PointerInteract (the click + pointer-event pads), MouseRegion (hover
//! cursor + enter/exit), and Focusable (key events + focus/blur), all
//! storing SEALED MUTATIONS — the dispatch enqueues the invocation and the
//! flush's mutation pass invokes the kit-sealed closure with the ctx + the
//! typed event (the plan's queued law) — plus the `focus_request` row.
//!
//! The payload shapes cross as native `Value` args (each event's
//! `MutationPayload::to_value_args`); the kit's dispatch entries
//! (`__tur_cb_m*`) construct the typed event values rut-side. The legacy
//! fn-box rails (on_tap / on_down / on_move / on_up, the two-id twins, and
//! the id / ids payload rails) died with the M2 corpus sweep.

use std::rc::Rc;

use crate::builtin_plugins::focus::FocusableView;
use crate::builtin_plugins::gesture::{
    MouseRegionView, PointerInteractEvent, PointerInteractView, PointerRegionEvent,
};
use crate::core::edgy::mutation::MutationHandle;
use crate::core::focus::{BlurEvent, FocusEvent};
use crate::core::layout::HitTestBehavior;
use crate::core::view::Val;
use crate::core::platform::key_event::{KeydownEvent, KeyupEvent};
use crate::core::rut_runtime::{RutHandles, RutView, readable_of};
use crate::core::shell::Cursor;

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

/// Declare the gesture rows + flag consts (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        // pointer interact
        row("pi_new", vec![], TY_OPAQUE),
        // The M2 mutation surface: every pad stores SEALED mutations
        // (`mutate` / `mutate_ev` mint them); the dispatch enqueues the
        // invocation and the flush's mutation pass invokes the closure
        // with the ctx + the typed event (the plan's queued law). The
        // legacy fn-box rows (pi_on_tap / on_down / on_move / on_up and
        // the id / ids payload rails) died with the M2 corpus sweep.
        row("pi_on_click", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_mut_down", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_mut_move", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_mut_up", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_on_context_menu", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_behavior", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("pi_build", vec![TY_OPAQUE], TY_OPAQUE),
        // mouse region
        row("mr_new", vec![], TY_OPAQUE),
        row("mr_on_enter", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_on_exit", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_cursor", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_cursor_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_behavior", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("mr_build", vec![TY_OPAQUE], TY_OPAQUE),
        // focusable
        row("focus_new", vec![], TY_OPAQUE),
        row("focus_on_key_down", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("focus_on_focus", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("focus_on_blur", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("focus_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("focus_build", vec![TY_OPAQUE], TY_OPAQUE),
        row("focus_request", vec![TY_U64], TY_NIL),
    ]);
    // (No flag consts: `HitTestBehavior` / `Cursor` are the kit's
    // name-only enums — the exhaustive `when` mappers in tur_kit are the
    // SOLE carriers of the row codes.)
}

use rut_core::types::{TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

// ---------------------------------------------------------------------------
// Family specs.
// ---------------------------------------------------------------------------

/// The PointerInteract spec (no id rails — the drag mutations capture
/// their state; events flow by the typed `PointerEvent`).
pub(crate) struct PiSpec {
    behavior: Option<Val<HitTestBehavior>>,
    query_key: Option<Vec<String>>,
    on_click: Option<MutationHandle<PointerInteractEvent>>,
    on_pointer_down: Option<MutationHandle<PointerInteractEvent>>,
    on_pointer_move: Option<MutationHandle<PointerInteractEvent>>,
    on_pointer_up: Option<MutationHandle<PointerInteractEvent>>,
    on_context_menu: Option<MutationHandle<PointerInteractEvent>>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

/// The MouseRegion spec.
pub(crate) struct MrSpec {
    behavior: Option<Val<HitTestBehavior>>,
    cursor: Option<Val<Cursor>>,
    on_enter: Option<MutationHandle<PointerRegionEvent>>,
    on_exit: Option<MutationHandle<PointerRegionEvent>>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

/// The Focusable spec.
pub(crate) struct FocusSpec {
    on_key_down: Option<MutationHandle<KeydownEvent>>,
    on_focus: Option<MutationHandle<FocusEvent>>,
    on_blur: Option<MutationHandle<BlurEvent>>,
    child: Option<Rc<dyn crate::core::view::View>>,
}

fn behavior_of(v: u64) -> HitTestBehavior {
    match v {
        1 => HitTestBehavior::Translucent,
        _ => HitTestBehavior::Opaque,
    }
}

/// Store a sealed mutation under a pad: the atom id is the crossing (the
/// ids ARE the atoms); the flush's mutation pass invokes the closure with
/// the ctx + the typed event.
fn pad_mutation<E>(atom: u64) -> Option<MutationHandle<E>> {
    Some(MutationHandle::<E>::new(crate::core::rut_runtime::mutation_of(atom)))
}

/// Install the gesture-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // ---- pointer interact -------------------------------------------------
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        let spec = PiSpec {
            behavior: None,
            query_key: None,
            on_click: None,
            on_pointer_down: None,
            on_pointer_move: None,
            on_pointer_up: None,
            on_context_menu: None,
            child: None,
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    // The mutation surface: each pad stores the SEALED mutation (the atom
    // id is the crossing — the closure the flush's mutation pass invokes
    // carries the kit adapter + the ctx wiring).
    rut_vm::pkg_fn!(pkg, "pi_on_click", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, atom: u64| {
        let m = pad_mutation::<PointerInteractEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_click = m)
    });
    rut_vm::pkg_fn!(pkg, "pi_mut_down", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, atom: u64| {
        let m = pad_mutation::<PointerInteractEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_pointer_down = m)
    });
    rut_vm::pkg_fn!(pkg, "pi_mut_move", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, atom: u64| {
        let m = pad_mutation::<PointerInteractEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_pointer_move = m)
    });
    rut_vm::pkg_fn!(pkg, "pi_mut_up", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, atom: u64| {
        let m = pad_mutation::<PointerInteractEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_pointer_up = m)
    });
    rut_vm::pkg_fn!(pkg, "pi_on_context_menu", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, atom: u64| {
        let m = pad_mutation::<PointerInteractEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_context_menu = m)
    });
    rut_vm::pkg_fn!(pkg, "pi_behavior", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.behavior = Some(Val::Static(behavior_of(v))))
    });
    rut_vm::pkg_fn!(pkg, "pi_qkey", (Opaque<PiSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "pi_child", (Opaque<PiSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "pi_build", (Opaque<PiSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>| {
        let view = b.with(|s| {
            Rc::new(PointerInteractView {
                behavior: s.behavior.clone(),
                on_click: s.on_click,
                on_pointer_down: s.on_pointer_down,
                on_pointer_move: s.on_pointer_move,
                on_pointer_up: s.on_pointer_up,
                on_context_menu: s.on_context_menu,
                query_key: s.query_key.clone(),
                child: s.child.clone(),
            }) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- mouse region -------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "mr_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        let spec = MrSpec { behavior: None, cursor: None, on_enter: None, on_exit: None, child: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "mr_on_enter", (Opaque<MrSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, atom: u64| {
        let m = pad_mutation::<PointerRegionEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_enter = m)
    });
    rut_vm::pkg_fn!(pkg, "mr_on_exit", (Opaque<MrSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, atom: u64| {
        let m = pad_mutation::<PointerRegionEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_exit = m)
    });
    // Static cursor (`0` = `Auto`).
    rut_vm::pkg_fn!(pkg, "mr_cursor", (Opaque<MrSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.cursor = Some(Val::Static(cursor_of(v))))
    });
    // Reactive cursor: resolved from a str atom each layout re-resolve
    // (`Cursor: FromValue` decodes the standard keyword).
    rut_vm::pkg_fn!(pkg, "mr_cursor_bound", (Opaque<MrSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| s.cursor = Some(Val::Reactive(readable_of::<Cursor>(atom))))
    });
    rut_vm::pkg_fn!(pkg, "mr_behavior", (Opaque<MrSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.behavior = Some(Val::Static(behavior_of(v))))
    });
    rut_vm::pkg_fn!(pkg, "mr_child", (Opaque<MrSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "mr_build", (Opaque<MrSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>| {
        let view = b.with(|s| {
            Rc::new(MouseRegionView {
                behavior: s.behavior.clone(),
                cursor: s.cursor.clone(),
                on_enter: s.on_enter,
                on_exit: s.on_exit,
                child: s.child.clone(),
            }) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- focusable ------------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "focus_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        let spec = FocusSpec { on_key_down: None, on_focus: None, on_blur: None, child: None };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "focus_on_key_down", (Opaque<FocusSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, atom: u64| {
        let m = pad_mutation::<KeydownEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_key_down = m)
    });
    rut_vm::pkg_fn!(pkg, "focus_on_focus", (Opaque<FocusSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, atom: u64| {
        let m = pad_mutation::<FocusEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_focus = m)
    });
    rut_vm::pkg_fn!(pkg, "focus_on_blur", (Opaque<FocusSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, atom: u64| {
        let m = pad_mutation::<BlurEvent>(atom);
        b.with_mut(vm, |_vm, s| s.on_blur = m)
    });
    rut_vm::pkg_fn!(pkg, "focus_child", (Opaque<FocusSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "focus_build", (Opaque<FocusSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>| {
        let view = b.with(|s| {
            Rc::new(FocusableView {
                on_key_down: s.on_key_down,
                on_key_up: None::<MutationHandle<KeyupEvent>>,
                on_focus: s.on_focus,
                on_blur: s.on_blur,
                child: s.child.clone(),
            }) as Rc<dyn crate::core::view::View>
        })?;
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


/// The crossing decode for a cursor (the `Cursor` variant by its code —
/// the consts' order, now carried kit-side by the `when` mapper).
fn cursor_of(code: u64) -> Cursor {
    use Cursor::*;
    match code {
        0 => Auto,
        2 => None,
        3 => ContextMenu,
        4 => Help,
        5 => Pointer,
        6 => Progress,
        7 => Wait,
        8 => Cell,
        9 => Crosshair,
        10 => Text,
        11 => VerticalText,
        12 => Alias,
        13 => Copy,
        14 => Move,
        15 => NoDrop,
        16 => NotAllowed,
        17 => Grab,
        18 => Grabbing,
        19 => EResize,
        20 => NResize,
        21 => NeResize,
        22 => NwResize,
        23 => SResize,
        24 => SeResize,
        25 => SwResize,
        26 => WResize,
        27 => EwResize,
        28 => NsResize,
        29 => NeswResize,
        30 => NwseResize,
        31 => ColResize,
        32 => RowResize,
        33 => AllScroll,
        34 => ZoomIn,
        35 => ZoomOut,
        _ => Default,
    }
}

