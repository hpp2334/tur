//! C1 — text-input rows: `Input` / editable-text over realm-minted
//! controllers.
//!
//! The engine's `EditableTextElement` keeps its text state in a
//! `TextEditingController` — a boa `JsData` payload reached through a
//! `JsObject`. The rut rail must share that EXACT object (keyboard / IME /
//! paste subsystems mutate it; the element renders it), so the rows mint
//! the controllers through the [`RutRealm`] face and hand rut an opaque
//! wrapper (`RutTextCtrl` / `RutUndo`). Method rows downcast through the
//! `JsObject` — no realm borrow needed on the accessor path.
//!
//! IME / paste / caret-visibility are engine subsystems that act on the
//! FOCUSED editable: once a rut `Input` exists and can take focus, they
//! work unchanged (the gate drives `send_key` / `send_ime` end to end).

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::JsValue;
use boa_engine::object::JsObject;

use crate::builtin_plugins::text::InputView;
use crate::builtin_plugins::text::controller::{SpanData, TextEditingController, UndoController};

/// A single unstyled span (the `set_text` / paste payload shape).
fn plain_span(text: &str) -> SpanData {
    SpanData {
        text: text.to_string(),
        weight: None,
        italic: false,
        underline: false,
        font_size: None,
        color: None,
    }
}
use rut_vm::Opaque;

use super::RutHandles;
use super::RutView;

#[allow(unused_imports)]
use rut_core::types::{TY_BOOL, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The opaque wrapper over a realm-minted `TextEditingController`.
pub struct RutTextCtrl(pub JsObject);

/// The opaque wrapper over a realm-minted `UndoController`.
pub struct RutUndoCtrl(pub JsObject);

/// Declare the C1 rows on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<TypeId>, TypeId)> {
    use rut_core::types::*;
    vec![
        ("tctrl_new", vec![], TY_OPAQUE),
        ("undo_new", vec![], TY_OPAQUE),
        (
            "el_input",
            vec![TY_OPAQUE, TY_OPAQUE, TY_STR, TY_F64, TY_F64],
            TY_OPAQUE,
        ),
        ("tctrl_text", vec![TY_OPAQUE], TY_STR),
        ("tctrl_set_text", vec![TY_OPAQUE, TY_STR], TY_NIL),
        ("tctrl_cursor", vec![TY_OPAQUE], TY_U64),
        (
            "tctrl_select",
            vec![TY_OPAQUE, TY_U64, TY_U64],
            TY_NIL,
        ),
        ("tctrl_clear", vec![TY_OPAQUE], TY_NIL),
        ("tctrl_paste", vec![TY_OPAQUE, TY_STR], TY_NIL),
        (
            "el_input_opts",
            vec![TY_OPAQUE, TY_OPAQUE, TY_STR, TY_F64, TY_F64, TY_U64],
            TY_OPAQUE,
        ),
        ("undo_can_undo", vec![TY_OPAQUE], TY_BOOL),
        ("undo_can_redo", vec![TY_OPAQUE], TY_BOOL),
        ("undo_clear", vec![TY_OPAQUE], TY_NIL),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

use rut_core::types::TypeId;

/// Install the C1 bodies.
pub fn install(
    pkg: &mut rut_vm::interp::HostPkg,
    handles: &Rc<RutHandles>,
) {
    // Mint a `TextEditingController` through the realm face. First demand
    // constructs the realm (the same deferred-replay path a JS load runs).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "tctrl_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = vm;
        let obj = h
            .realm
            .with_realm(|boa| {
                let data = TextEditingController::data_constructor(&JsValue::undefined(), &[], boa)?;
                TextEditingController::from_data(data, boa)
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("tctrl_new: {e}")))?
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("tctrl_new: {e}")))?;
        Ok(Opaque::alloc(vm, RutTextCtrl(obj))?.handle().clone())
    });

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "undo_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = vm;
        let obj = h
            .realm
            .with_realm(|boa| {
                let data = UndoController::data_constructor(&JsValue::undefined(), &[], boa)?;
                UndoController::from_data(data, boa)
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("undo_new: {e}")))?
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("undo_new: {e}")))?;
        Ok(Opaque::alloc(vm, RutUndoCtrl(obj))?.handle().clone())
    });

    // An `Input` bound to the realm-minted controllers (the undo slot rides
    // the same spec; `EditableText::build` attaches the recorder).
    rut_vm::pkg_fn!(pkg, "el_input", (Opaque<RutTextCtrl>, Opaque<RutUndoCtrl>, &str, f64, f64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutTextCtrl>, undo: Opaque<RutUndoCtrl>, placeholder: &str, width: f64, height: f64| {
        let ctrl = ctrl.with(|c| c.0.clone())?;
        let undo = undo.with(|u| u.0.clone())?;
        let view = Rc::new(InputView::new_rut(
            ctrl,
            Some(undo),
            if placeholder.is_empty() { None } else { Some(placeholder.to_string()) },
            if width > 0.0 { Some(width) } else { None },
            if height > 0.0 { Some(height) } else { None },
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // Input with option flags — bit 0 = multiline, bit 1 = obscure (the
    // password twin), bit 2 = center alignment of the wrapper.
    let h2 = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_input_opts", (Opaque<RutTextCtrl>, Opaque<RutUndoCtrl>, &str, f64, f64, u64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutTextCtrl>, undo: Opaque<RutUndoCtrl>, placeholder: &str, width: f64, height: f64, flags: u64| {
        let _ = &h2;
        let ctrl = ctrl.with(|c| c.0.clone())?;
        let undo = undo.with(|u| u.0.clone())?;
        let view = InputView::new_rut_opts(
            ctrl,
            Some(undo),
            if placeholder.is_empty() { None } else { Some(placeholder.to_string()) },
            if width > 0.0 { Some(width) } else { None },
            if height > 0.0 { Some(height) } else { None },
            flags & 1 != 0,
            flags & 2 != 0,
        );
        Ok(Opaque::alloc(vm, RutView(Rc::new(view)))?.handle().clone())
    });

    // ---- controller method rows (downcast without a realm borrow) -------
    rut_vm::pkg_fn!(pkg, "tctrl_text", (Opaque<RutTextCtrl>,) -> String, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with(|c| Ok(c
            .0
            .downcast_ref::<TextEditingController>()
            .map(|t| t.text())
            .unwrap_or_default()))?
    });
    rut_vm::pkg_fn!(pkg, "tctrl_set_text", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        c.with_mut(vm, |_vm, c| {
            if let Some(mut t) = c.0.downcast_mut::<TextEditingController>() {
                t.set_spans(vec![plain_span(text)]);
            }
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "tctrl_cursor", (Opaque<RutTextCtrl>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with(|c| Ok(c
            .0
            .downcast_ref::<TextEditingController>()
            .map(|t| t.cursor_position() as u64)
            .unwrap_or(0)))?
    });
    rut_vm::pkg_fn!(pkg, "tctrl_select", (Opaque<RutTextCtrl>, u64, u64) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, anchor: u64, end: u64| {
        c.with_mut(vm, |_vm, c| {
            if let Some(mut t) = c.0.downcast_mut::<TextEditingController>() {
                t.set_selection(anchor as usize, end as usize);
            }
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "tctrl_clear", (Opaque<RutTextCtrl>,) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with_mut(vm, |_vm, c| {
            if let Some(mut t) = c.0.downcast_mut::<TextEditingController>() {
                t.clear();
            }
        })?;
        Ok(())
    });
    // The paste-equivalent: insert at the caret, replacing any selection —
    // the same state change the engine's ClipboardPasteSubsystem applies.
    rut_vm::pkg_fn!(pkg, "tctrl_paste", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        c.with_mut(vm, |_vm, c| {
            if let Some(mut t) = c.0.downcast_mut::<TextEditingController>() {
                t.delete_selection();
                let pos = t.cursor_position();
                t.insert_str_at(pos, text);
                t.set_cursor_position(pos + text.len());
            }
        })?;
        Ok(())
    });

    // ---- undo controller rows -------------------------------------------
    rut_vm::pkg_fn!(pkg, "undo_can_undo", (Opaque<RutUndoCtrl>,) -> bool, move |_vm: &mut rut_vm::interp::Vm, u: Opaque<RutUndoCtrl>| {
        u.with(|u| Ok(u
            .0
            .downcast_ref::<UndoController>()
            .is_some_and(|u| u.can_undo())))?
    });
    rut_vm::pkg_fn!(pkg, "undo_can_redo", (Opaque<RutUndoCtrl>,) -> bool, move |_vm: &mut rut_vm::interp::Vm, u: Opaque<RutUndoCtrl>| {
        u.with(|u| Ok(u
            .0
            .downcast_ref::<UndoController>()
            .is_some_and(|u| u.can_redo())))?
    });
    rut_vm::pkg_fn!(pkg, "undo_clear", (Opaque<RutUndoCtrl>,) -> (), move |vm: &mut rut_vm::interp::Vm, u: Opaque<RutUndoCtrl>| {
        u.with_mut(vm, |_vm, u| {
            if let Some(mut u) = u.0.downcast_mut::<UndoController>() {
                u.clear();
            }
        })?;
        Ok(())
    });
}
