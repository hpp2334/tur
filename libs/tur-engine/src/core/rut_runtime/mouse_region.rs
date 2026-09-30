//! Phase-4 corpus rows — `MouseRegion`: hover cursor + enter/exit
//! callbacks on the intent rail, and the hit-test behavior flag.
//!
//! - `el_mouse_region(id, on_enter, on_exit, cursor, behavior, child)` —
//!   static cursor (`""`-style `0` = `Auto`).
//! - `el_mouse_region_bound(id, on_enter, on_exit, cursor_atom, behavior,
//!   child)` — the cursor resolves from a str atom each layout (the
//!   reactive-cursor corpus case).
//!
//! Enter/exit callbacks are `(id, seq)`-shaped `entry fn`s on the Value
//! intent rail (the payload is the monotonic event count).

use std::rc::Rc;

use crate::builtin_plugins::gesture::MouseRegionView;
use crate::core::layout::HitTestBehavior;
use crate::core::shell::Cursor;
use crate::core::view::Val;

use rut_vm::Opaque;

use super::{Intent, RutView};

pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        (
            "el_mouse_region",
            vec![TY_U64, TY_STR, TY_STR, TY_U64, TY_U64, TY_OPAQUE],
            TY_OPAQUE,
        ),
        (
            "el_mouse_region_bound",
            vec![TY_U64, TY_STR, TY_STR, TY_U64, TY_U64, TY_OPAQUE],
            TY_OPAQUE,
        ),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// The cursor flag consts — `Cursor` variant order (the doc'd crossing).
pub fn decl_consts() -> Vec<(String, rut_core::types::TypeId, u64)> {
    use rut_core::types::TY_U64;
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
    order
        .into_iter()
        .map(|(name, c)| (name.to_string(), TY_U64, cursor_code(c)))
        .collect()
}

/// The u64 crossing code for a cursor (its `Cursor` variant position).
pub fn cursor_code(c: Cursor) -> u64 {
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

fn behavior_of(v: u64) -> HitTestBehavior {
    match v {
        1 => HitTestBehavior::Translucent,
        _ => HitTestBehavior::Opaque,
    }
}

/// Install the bodies. `behavior` consts: `0` = Opaque (default), `1` =
/// Translucent.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<super::RutHandles>) {
    // Static cursor.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_mouse_region", (u64, &str, &str, u64, u64, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64, on_enter: &str, on_exit: &str, cursor: u64, behavior: u64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(MouseRegionView {
            behavior: Some(Val::Static(behavior_of(behavior))),
            cursor: Some(Val::Static(cursor_of(cursor))),
            on_enter: region_mutation(&h, id, on_enter),
            on_exit: region_mutation(&h, id, on_exit),
            child: Some(child),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // Reactive cursor: resolved from a str atom each layout re-resolve
    // (`Cursor: FromValue` decodes the standard keyword).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_mouse_region_bound", (u64, &str, &str, u64, u64, Opaque<RutView>) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id: u64, on_enter: &str, on_exit: &str, cursor_atom: u64, behavior: u64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(MouseRegionView {
            behavior: Some(Val::Static(behavior_of(behavior))),
            cursor: Some(Val::Reactive(
                crate::core::edgy::reactive::Readable::Source(
                    crate::core::edgy::reactive::Source::<Cursor>::from_id(
                        crate::core::edgy::reactive::AtomId(cursor_atom as u32),
                    ),
                ),
            )),
            on_enter: region_mutation(&h, id, on_enter),
            on_exit: region_mutation(&h, id, on_exit),
            child: Some(child),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}

/// Build the enter/exit mutation: pushes a Value intent drained into the
/// named `entry fn(id, seq)`.
fn region_mutation(
    handles: &Rc<super::RutHandles>,
    id: u64,
    name: &str,
) -> Option<crate::core::edgy::mutation::MutationHandle<crate::builtin_plugins::gesture::PointerRegionEvent>> {
    if name.is_empty() {
        return None;
    }
    let h = handles.clone();
    let cb = name.to_string();
    let mutation = h.store.bridge().build_mutate(move |_bridge, _args, _boa| {
        let n = h.click_seq.get() + 1;
        h.click_seq.set(n);
        h.pending_calls
            .borrow_mut()
            .push(Intent::Value {
                name: cb.clone(),
                a: id,
                value: crate::core::edgy::Value::Num(n as f64),
            });
        h.dirty.set(true);
        Ok(crate::core::edgy::Value::Nil)
    });
    Some(crate::core::edgy::mutation::MutationHandle::<
        crate::builtin_plugins::gesture::PointerRegionEvent,
    >::new(mutation))
}
