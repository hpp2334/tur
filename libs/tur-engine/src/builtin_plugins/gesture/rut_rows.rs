//! The gesture families' `tur` host-pkg rows (via the pkg-extension seam):
//! PointerInteract (the tap + pointer-event pads), MouseRegion (hover
//! cursor + enter/exit), and Focusable (key events + focus/blur), all
//! reporting through the intent-queue rail, plus the `focus_request` row.
//!
//! Payloads beyond the legacy `(u64, u64, f64)` click shape cross as
//! intent records ([`Intent::Pointer`] / [`Intent::Pointer2`] /
//! [`Intent::Key`]) — the drain dispatches each shape into its `entry fn`
//! signature:
//!
//! - tap callbacks (the historical `el_button` shape):
//!   `entry fn cb(id_a: u64, id_b: u64, seq: f64)`
//! - pointer callbacks (single-id rail):
//!   `entry fn cb(id: u64, lx: f64, ly: f64, gx: f64, gy: f64, button: u64)`
//!   (`button` is 0 for down/move/up/click, 2 for the context-menu's right
//!   button); the two-id rail delivers
//!   `entry fn cb(a: u64, b: u64, lx: f64, ly: f64, gx: f64, gy: f64, button: u64)`
//! - key callback: `entry fn cb(id: u64, key: str, code: str, mods: u64,
//!   kind: u64)` — `mods` bit0 shift / bit1 ctrl / bit2 alt / bit3 meta,
//!   `kind` 0 = down, 1 = up
//! - focus/blur callbacks: `entry fn cb(id: u64, b: u64, n: f64)`

use std::rc::Rc;

use crate::builtin_plugins::focus::FocusableView;
use crate::builtin_plugins::gesture::{
    MouseRegionView, PointerInteractEvent, PointerInteractView, PointerRegionEvent,
};
use crate::core::edgy::mutation::MutationHandle;
use crate::core::edgy::value::Value;
use crate::core::focus::{BlurEvent, FocusEvent};
use crate::core::layout::HitTestBehavior;
use crate::core::view::Val;
use crate::core::platform::key_event::{KeydownEvent, KeyupEvent};
use crate::core::rut_runtime::{Intent, RutHandles, RutView, readable_of};
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
        row("pi_id", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_ids", vec![TY_OPAQUE, TY_U64, TY_U64], TY_NIL),
        row("pi_on_tap", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_on_click", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_on_down", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_on_move", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_on_up", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_on_context_menu", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_behavior", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("pi_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("pi_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("pi_build", vec![TY_OPAQUE], TY_OPAQUE),
        // mouse region
        row("mr_new", vec![], TY_OPAQUE),
        row("mr_on_enter", vec![TY_OPAQUE, TY_STR, TY_U64], TY_NIL),
        row("mr_on_exit", vec![TY_OPAQUE, TY_STR, TY_U64], TY_NIL),
        row("mr_cursor", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_cursor_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_behavior", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("mr_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("mr_build", vec![TY_OPAQUE], TY_OPAQUE),
        // focusable
        row("focus_new", vec![], TY_OPAQUE),
        row("focus_on_key_down", vec![TY_OPAQUE, TY_STR, TY_U64], TY_NIL),
        row("focus_on_focus", vec![TY_OPAQUE, TY_STR, TY_U64], TY_NIL),
        row("focus_on_blur", vec![TY_OPAQUE, TY_STR, TY_U64], TY_NIL),
        row("focus_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("focus_build", vec![TY_OPAQUE], TY_OPAQUE),
        row("focus_request", vec![TY_U64], TY_NIL),
    ]);
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    let order = [
        ("CURSOR_AUTO", Cursor::Auto),
        ("CURSOR_DEFAULT", Cursor::Default),
        ("CURSOR_NONE", Cursor::None),
        ("CURSOR_CONTEXT_MENU", Cursor::ContextMenu),
        ("CURSOR_HELP", Cursor::Help),
        ("CURSOR_POINTER", Cursor::Pointer),
        ("CURSOR_PROGRESS", Cursor::Progress),
        ("CURSOR_WAIT", Cursor::Wait),
        ("CURSOR_CELL", Cursor::Cell),
        ("CURSOR_CROSSHAIR", Cursor::Crosshair),
        ("CURSOR_TEXT", Cursor::Text),
        ("CURSOR_MOVE", Cursor::Move),
        ("CURSOR_GRAB", Cursor::Grab),
        ("CURSOR_GRABBING", Cursor::Grabbing),
        ("CURSOR_E_RESIZE", Cursor::EResize),
        ("CURSOR_W_RESIZE", Cursor::WResize),
        ("CURSOR_EW_RESIZE", Cursor::EwResize),
        ("CURSOR_NS_RESIZE", Cursor::NsResize),
        ("CURSOR_COL_RESIZE", Cursor::ColResize),
        ("CURSOR_ROW_RESIZE", Cursor::RowResize),
        ("CURSOR_ALL_SCROLL", Cursor::AllScroll),
    ];
    let mut consts = vec![
        // HitTestBehavior (0 = Opaque default, 1 = Translucent)
        c("HIT_TEST_OPAQUE", 0),
        c("HIT_TEST_TRANSLUCENT", 1),
    ];
    consts.extend(
        order
            .into_iter()
            .map(|(name, cur)| (name.to_string(), TY_U64, cursor_code(cur))),
    );
    cx.consts.extend(consts);
}

use rut_core::types::{TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

// ---------------------------------------------------------------------------
// Family specs.
// ---------------------------------------------------------------------------

/// The PointerInteract spec. `two_ids` selects the callback payload rail
/// (single-id `Intent::Pointer` vs two-id `Intent::Pointer2`).
pub(crate) struct PiSpec {
    id_a: u64,
    id_b: u64,
    two_ids: bool,
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

// ---------------------------------------------------------------------------
// Callback mutation builders (the intent-queue rail).
// ---------------------------------------------------------------------------

/// Queue a tap intent (the historical click shape):
/// `(name, id_a, id_b, seq)`.
fn tap_mutation(handles: &Rc<RutHandles>, id_a: u64, id_b: u64, cb: &str) -> Option<MutationHandle<PointerInteractEvent>> {
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

/// Queue a pointer intent for `cb` (empty name = absent). The mutation
/// receives the pointer payload through the native crossing
/// (`[local.x, local.y, global.x, global.y]`); `two_ids` selects the
/// two-id `Intent::Pointer2` rail.
fn pointer_mutation(
    handles: &Rc<RutHandles>,
    id_a: u64,
    id_b: u64,
    two_ids: bool,
    button: u64,
    cb: &str,
) -> Option<MutationHandle<PointerInteractEvent>> {
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
        if two_ids {
            h.pending_calls.borrow_mut().push(Intent::Pointer2 {
                name: name.clone(),
                a: id_a,
                b: id_b,
                lx,
                ly,
                gx,
                gy,
                button,
            });
        } else {
            h.pending_calls.borrow_mut().push(Intent::Pointer {
                name: name.clone(),
                id: id_a,
                lx,
                ly,
                gx,
                gy,
                button,
            });
        }
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

/// A focus intent (no payload — the id is baked into the closure).
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

/// Build the enter/exit mutation: pushes a Click-shaped intent (the
/// `(report, id, seq)` drain shape) into the named `entry fn(a, b, n)`.
fn region_mutation(
    handles: &Rc<RutHandles>,
    id: u64,
    name: &str,
) -> Option<MutationHandle<PointerRegionEvent>> {
    if name.is_empty() {
        return None;
    }
    let h = handles.clone();
    let cb = name.to_string();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
        let n = h.click_seq.get() + 1;
        h.click_seq.set(n);
        h.pending_calls
            .borrow_mut()
            .push(Intent::Click {
                name: cb.clone(),
                a: id,
                b: id,
                seq: n as f64,
            });
        h.dirty.set(true);
        Ok(crate::core::edgy::Value::Nil)
    });
    Some(MutationHandle::<PointerRegionEvent>::new(mutation))
}

/// Install the gesture-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // ---- pointer interact -------------------------------------------------
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        let spec = PiSpec {
            id_a: 0,
            id_b: 0,
            two_ids: false,
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
    // Single-id rail: pointer callbacks deliver `(id, lx, ly, gx, gy, btn)`.
    rut_vm::pkg_fn!(pkg, "pi_id", (Opaque<PiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, id: u64| {
        b.with_mut(vm, |_vm, s| {
            s.id_a = id;
            s.two_ids = false;
        })
    });
    // Two-id rail: pointer callbacks deliver `(a, b, lx, ly, gx, gy, btn)`;
    // taps still deliver the `(a, b, seq)` click shape.
    rut_vm::pkg_fn!(pkg, "pi_ids", (Opaque<PiSpec>, u64, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, a: u64, bb: u64| {
        b.with_mut(vm, |_vm, s| {
            s.id_a = a;
            s.id_b = bb;
            s.two_ids = true;
        })
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_on_tap", (Opaque<PiSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, cb: &str| {
        let (a, bb) = b.with(|s| (s.id_a, s.id_b))?;
        let m = tap_mutation(&h, a, bb, cb);
        b.with_mut(vm, |_vm, s| s.on_click = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_on_click", (Opaque<PiSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, cb: &str| {
        let (a, bb, two) = b.with(|s| (s.id_a, s.id_b, s.two_ids))?;
        let m = pointer_mutation(&h, a, bb, two, 0, cb);
        b.with_mut(vm, |_vm, s| s.on_click = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_on_down", (Opaque<PiSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, cb: &str| {
        let (a, bb, two) = b.with(|s| (s.id_a, s.id_b, s.two_ids))?;
        let m = pointer_mutation(&h, a, bb, two, 0, cb);
        b.with_mut(vm, |_vm, s| s.on_pointer_down = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_on_move", (Opaque<PiSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, cb: &str| {
        let (a, bb, two) = b.with(|s| (s.id_a, s.id_b, s.two_ids))?;
        let m = pointer_mutation(&h, a, bb, two, 0, cb);
        b.with_mut(vm, |_vm, s| s.on_pointer_move = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_on_up", (Opaque<PiSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, cb: &str| {
        let (a, bb, two) = b.with(|s| (s.id_a, s.id_b, s.two_ids))?;
        let m = pointer_mutation(&h, a, bb, two, 0, cb);
        b.with_mut(vm, |_vm, s| s.on_pointer_up = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "pi_on_context_menu", (Opaque<PiSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<PiSpec>, cb: &str| {
        let (a, bb, two) = b.with(|s| (s.id_a, s.id_b, s.two_ids))?;
        let m = pointer_mutation(&h, a, bb, two, 2, cb);
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
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mr_on_enter", (Opaque<MrSpec>, &str, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, cb: &str, id: u64| {
        let m = region_mutation(&h, id, cb);
        b.with_mut(vm, |_vm, s| s.on_enter = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mr_on_exit", (Opaque<MrSpec>, &str, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<MrSpec>, cb: &str, id: u64| {
        let m = region_mutation(&h, id, cb);
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
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "focus_on_key_down", (Opaque<FocusSpec>, &str, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, cb: &str, id: u64| {
        let m = key_mutation(&h, id, cb);
        b.with_mut(vm, |_vm, s| s.on_key_down = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "focus_on_focus", (Opaque<FocusSpec>, &str, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, cb: &str, id: u64| {
        let m = focus_mutation(&h, id, cb);
        b.with_mut(vm, |_vm, s| s.on_focus = m)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "focus_on_blur", (Opaque<FocusSpec>, &str, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<FocusSpec>, cb: &str, id: u64| {
        let m = blur_mutation(&h, id, cb);
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

/// The u64 crossing code for a cursor (its `Cursor` variant position).
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

/// The u64 crossing code for a cursor (its `Cursor` variant position).
fn cursor_code(c: Cursor) -> u64 {
    use Cursor::*;
    match c {
        Auto => 0,
        Default => 1,
        None => 2,
        ContextMenu => 3,
        Help => 4,
        Pointer => 5,
        Progress => 6,
        Wait => 7,
        Cell => 8,
        Crosshair => 9,
        Text => 10,
        VerticalText => 11,
        Alias => 12,
        Copy => 13,
        Move => 14,
        NoDrop => 15,
        NotAllowed => 16,
        Grab => 17,
        Grabbing => 18,
        EResize => 19,
        NResize => 20,
        NeResize => 21,
        NwResize => 22,
        SResize => 23,
        SeResize => 24,
        SwResize => 25,
        WResize => 26,
        EwResize => 27,
        NsResize => 28,
        NeswResize => 29,
        NwseResize => 30,
        ColResize => 31,
        RowResize => 32,
        AllScroll => 33,
        ZoomIn => 34,
        ZoomOut => 35,
    }
}
