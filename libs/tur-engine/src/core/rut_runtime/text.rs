//! C1 — text-input rows: `Input` / editable-text over shared controllers.
//!
//! The engine's `EditableTextElement` keeps its text state in a
//! `TextEditingController`. The rut rail must share that EXACT controller
//! (keyboard / IME / paste subsystems mutate it; the element renders it),
//! so the rows mint `Rc<RefCell<…>>` controllers and hand rut an opaque
//! wrapper (`RutTextCtrl` / `RutUndoCtrl`). Method rows borrow through the
//! `Rc` — no runtime borrow beyond the row call.
//!
//! IME / paste / caret-visibility are engine subsystems that act on the
//! FOCUSED editable: once a rut `Input` exists and can take focus, they
//! work unchanged (the gate drives `send_key` / `send_ime` end to end).

use std::cell::RefCell;
use std::rc::Rc;

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

/// The opaque wrapper over a shared `TextEditingController`.
pub struct RutTextCtrl(pub Rc<RefCell<TextEditingController>>);

/// The opaque wrapper over a shared `UndoController`.
pub struct RutUndoCtrl(pub Rc<RefCell<UndoController>>);

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
        ("tctrl_push_span", vec![TY_OPAQUE, TY_STR], TY_NIL),
        ("tctrl_delete_selection", vec![TY_OPAQUE], TY_NIL),
        ("tctrl_insert_text", vec![TY_OPAQUE, TY_STR], TY_NIL),
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
        ("el_input_new", vec![], TY_OPAQUE),
        (
            "el_input_ctrl",
            vec![TY_OPAQUE, TY_F64, TY_F64, TY_F64],
            TY_OPAQUE,
        ),
        ("input_size", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        ("input_placeholder", vec![TY_OPAQUE, TY_STR], TY_NIL),
        ("input_color", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("input_placeholder_color", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("input_font_size", vec![TY_OPAQUE, TY_F64], TY_NIL),
        // the password twin's surface: the obscure toggle + the configurable
        // obscuring character (the JS `obscureText` / `obscuringCharacter`).
        ("input_obscure", vec![TY_OPAQUE, TY_BOOL], TY_NIL),
        ("input_obscure_char", vec![TY_OPAQUE, TY_STR], TY_NIL),
        // the multiline toggle on the builder surface (the JS
        // `multiline: true` twin — `el_input_opts` bit 0 is the view-form).
        ("input_multiline", vec![TY_OPAQUE, TY_BOOL], TY_NIL),
        // the font family (the JS `Input().fontFamily(...)` twin — the code
        // editor pins `"monospace"` so caret math sees uniform glyphs).
        ("input_font_family", vec![TY_OPAQUE, TY_STR], TY_NIL),
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
    // Mint a shared `TextEditingController` (plain Rust state — the
    // keyboard / IME / paste subsystems and the element all reach the same
    // Rc).
    rut_vm::pkg_fn!(pkg, "tctrl_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let ctrl = Rc::new(RefCell::new(TextEditingController::new()));
        Ok(Opaque::alloc(vm, RutTextCtrl(ctrl))?.handle().clone())
    });

    rut_vm::pkg_fn!(pkg, "undo_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let undo = Rc::new(RefCell::new(UndoController::new()));
        Ok(Opaque::alloc(vm, RutUndoCtrl(undo))?.handle().clone())
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

    // ---- the input builder (style setter rows + el_build) ---------------
    rut_vm::pkg_fn!(pkg, "el_input_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, super::ViewBuilder::Input(Box::new(
            crate::builtin_plugins::text::InputView::empty_rut(),
        )))?.handle().clone())
    });
    // An input builder pre-bound to a controller (+ size + font size) —
    // the editor-shaped constructor (the huge-document fixture). 0 = absent
    // for every optional dim (the `el_input` row's semantics).
    rut_vm::pkg_fn!(pkg, "el_input_ctrl", (Opaque<RutTextCtrl>, f64, f64, f64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, ctrl: Opaque<RutTextCtrl>, w: f64, h: f64, font: f64| {
        let shared = ctrl.with(|c| c.0.clone())?;
        let mut spec = crate::builtin_plugins::text::InputView::empty_rut();
        spec.set_controller(shared);
        if w > 0.0 {
            spec.set_width(w);
        }
        if h > 0.0 {
            spec.set_height(h);
        }
        if font > 0.0 {
            spec.set_font_size(font);
        }
        Ok(Opaque::alloc(vm, super::ViewBuilder::Input(Box::new(spec)))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "input_size", (Opaque<super::ViewBuilder>, f64, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, w: f64, h: f64| {
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_width(w);
                spec.set_height(h);
            }
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "input_placeholder", (Opaque<super::ViewBuilder>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, text: &str| {
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_placeholder_str(text.to_string());
            }
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "input_color", (Opaque<super::ViewBuilder>, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, packed: u64| {
        let c = super::color_of(packed);
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_color(c);
            }
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "input_placeholder_color", (Opaque<super::ViewBuilder>, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, packed: u64| {
        let c = super::color_of(packed);
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_placeholder_color(c);
            }
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "input_font_size", (Opaque<super::ViewBuilder>, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, v: f64| {
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_font_size(v);
            }
        })?;
        Ok(())
    });
    // input_obscure(builder, on) — the password toggle (the JS
    // `obscureText: true` twin).
    rut_vm::pkg_fn!(pkg, "input_obscure", (Opaque<super::ViewBuilder>, bool) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, on: bool| {
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_obscure(on);
            }
        })?;
        Ok(())
    });
    // input_obscure_char(builder, ch) — the configurable obscuring
    // character (the JS `obscuringCharacter: '*'` twin; a multi-char str
    // takes its first char, matching the engine's per-char mask).
    rut_vm::pkg_fn!(pkg, "input_obscure_char", (Opaque<super::ViewBuilder>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, ch: &str| {
        let ch = ch.chars().next().unwrap_or('\u{2022}').to_string();
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_obscuring_character_str(ch);
            }
        })?;
        Ok(())
    });
    // input_font_family(builder, family) — the JS `fontFamily(...)` twin
    // (parley generic families: "monospace" / "serif" / sans-serif default).
    rut_vm::pkg_fn!(pkg, "input_font_family", (Opaque<super::ViewBuilder>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, family: &str| {
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_font_family_str(family.to_string());
            }
        })?;
        Ok(())
    });
    // input_multiline(builder, on) — the JS `multiline: true` twin (the
    // builder-form of `el_input_opts` bit 0).
    rut_vm::pkg_fn!(pkg, "input_multiline", (Opaque<super::ViewBuilder>, bool) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<super::ViewBuilder>, on: bool| {
        b.with_mut(vm, |_vm, b| {
            if let super::ViewBuilder::Input(spec) = &mut *b {
                spec.set_multiline(on);
            }
        })?;
        Ok(())
    });

    // ---- controller method rows (downcast without a realm borrow) -------
    rut_vm::pkg_fn!(pkg, "tctrl_text", (Opaque<RutTextCtrl>,) -> String, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with(|c| Ok(c.0.borrow().text()))?
    });
    rut_vm::pkg_fn!(pkg, "tctrl_set_text", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        c.with_mut(vm, |_vm, c| {
            c.0.borrow_mut().set_spans(vec![plain_span(text)]);
        })?;
        Ok(())
    });
    // Delete the selection (or nothing at an empty caret) — the undo
    // fixture's delete step.
    rut_vm::pkg_fn!(pkg, "tctrl_delete_selection", (Opaque<RutTextCtrl>,) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with_mut(vm, |_vm, c| {
            c.0.borrow_mut().delete_selection();
        })?;
        Ok(())
    });
    // Insert text at the caret (no selection replace) — the undo fixture's
    // typed-insert step.
    rut_vm::pkg_fn!(pkg, "tctrl_insert_text", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        c.with_mut(vm, |_vm, c| {
            let mut t = c.0.borrow_mut();
            let pos = t.cursor_position();
            t.insert_str_at(pos, text);
            t.set_cursor_position(pos + text.len());
        })?;
        Ok(())
    });

    // Append one plain span (the huge-document fixture's authoring loop).
    rut_vm::pkg_fn!(pkg, "tctrl_push_span", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        c.with_mut(vm, |_vm, c| {
            let mut spans = c.0.borrow().spans().to_vec();
            spans.push(plain_span(text));
            c.0.borrow_mut().set_spans_preserve_cursor(spans);
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "tctrl_cursor", (Opaque<RutTextCtrl>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with(|c| Ok(c.0.borrow().cursor_position() as u64))?
    });
    rut_vm::pkg_fn!(pkg, "tctrl_select", (Opaque<RutTextCtrl>, u64, u64) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, anchor: u64, end: u64| {
        c.with_mut(vm, |_vm, c| {
            c.0.borrow_mut().set_selection(anchor as usize, end as usize);
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "tctrl_clear", (Opaque<RutTextCtrl>,) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with_mut(vm, |_vm, c| {
            c.0.borrow_mut().clear();
        })?;
        Ok(())
    });
    // The paste-equivalent: insert at the caret, replacing any selection —
    // the same state change the engine's ClipboardPasteSubsystem applies.
    rut_vm::pkg_fn!(pkg, "tctrl_paste", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        c.with_mut(vm, |_vm, c| {
            let mut t = c.0.borrow_mut();
            t.delete_selection();
            let pos = t.cursor_position();
            t.insert_str_at(pos, text);
            t.set_cursor_position(pos + text.len());
        })?;
        Ok(())
    });

    // ---- undo controller rows -------------------------------------------
    rut_vm::pkg_fn!(pkg, "undo_can_undo", (Opaque<RutUndoCtrl>,) -> bool, move |_vm: &mut rut_vm::interp::Vm, u: Opaque<RutUndoCtrl>| {
        u.with(|u| Ok(u.0.borrow().can_undo()))?
    });
    rut_vm::pkg_fn!(pkg, "undo_can_redo", (Opaque<RutUndoCtrl>,) -> bool, move |_vm: &mut rut_vm::interp::Vm, u: Opaque<RutUndoCtrl>| {
        u.with(|u| Ok(u.0.borrow().can_redo()))?
    });
    rut_vm::pkg_fn!(pkg, "undo_clear", (Opaque<RutUndoCtrl>,) -> (), move |vm: &mut rut_vm::interp::Vm, u: Opaque<RutUndoCtrl>| {
        u.with_mut(vm, |_vm, u| {
            u.0.borrow_mut().clear();
        })?;
        Ok(())
    });
}
