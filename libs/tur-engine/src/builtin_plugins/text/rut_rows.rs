//! The text families' `tur` host-pkg rows (via the pkg-extension seam):
//! the Text spec family (literal / bound / derived + the style setters +
//! rich-text spans), the Input spec family over shared controllers, and the
//! realm-free controller method rows (`tctrl_*` / `undo_*`).
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

use crate::builtin_plugins::text::controller::{SpanData, TextEditingController, UndoController};
use crate::builtin_plugins::text::elements::paragraph::TextOverflow;
use crate::builtin_plugins::text::{InputView, TextView};
use crate::core::edgy::reactive::{AtomId, Derived, Readable, Source};
use crate::core::rut_runtime::{RutHandles, RutView, color_of};
use crate::core::view::Val;

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

/// Declare the text rows + span flag consts (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    cx.decl.extend(vec![
        // text family
        ("text_new".to_string(), vec![], TY_OPAQUE, false),
        ("text_text".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("text_bind".to_string(), vec![TY_OPAQUE, TY_U64], TY_NIL, false),
        ("text_bind_derived".to_string(), vec![TY_OPAQUE, TY_U64], TY_NIL, false),
        ("text_size".to_string(), vec![TY_OPAQUE, TY_F64], TY_NIL, false),
        ("text_weight".to_string(), vec![TY_OPAQUE, TY_F64], TY_NIL, false),
        ("text_color".to_string(), vec![TY_OPAQUE, TY_U64], TY_NIL, false),
        ("text_max_lines".to_string(), vec![TY_OPAQUE, TY_U64], TY_NIL, false),
        ("text_clip".to_string(), vec![TY_OPAQUE], TY_NIL, false),
        ("text_ellipsis".to_string(), vec![TY_OPAQUE], TY_NIL, false),
        ("text_overflow_visible".to_string(), vec![TY_OPAQUE], TY_NIL, false),
        ("text_selectable".to_string(), vec![TY_OPAQUE, TY_BOOL], TY_NIL, false),
        ("text_qkey".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("text_build".to_string(), vec![TY_OPAQUE], TY_OPAQUE, false),
        // rich-text spans
        ("spans_new".to_string(), vec![], TY_OPAQUE, false),
        (
            "span_add".to_string(),
            vec![TY_OPAQUE, TY_STR, TY_F64, TY_U64, TY_F64, TY_U64],
            TY_NIL,
            false,
        ),
        ("text_spans".to_string(), vec![TY_OPAQUE, TY_OPAQUE], TY_NIL, false),
        // input family
        ("input_new".to_string(), vec![], TY_OPAQUE, false),
        ("input_controller".to_string(), vec![TY_OPAQUE, TY_OPAQUE], TY_NIL, false),
        ("input_undo".to_string(), vec![TY_OPAQUE, TY_OPAQUE], TY_NIL, false),
        ("input_size".to_string(), vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL, false),
        ("input_placeholder".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("input_color".to_string(), vec![TY_OPAQUE, TY_U64], TY_NIL, false),
        ("input_placeholder_color".to_string(), vec![TY_OPAQUE, TY_U64], TY_NIL, false),
        ("input_font_size".to_string(), vec![TY_OPAQUE, TY_F64], TY_NIL, false),
        // the password twin's surface: the obscure toggle + the configurable
        // obscuring character (the JS `obscureText` / `obscuringCharacter`).
        ("input_obscure".to_string(), vec![TY_OPAQUE, TY_BOOL], TY_NIL, false),
        ("input_obscure_char".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        // the multiline toggle (the JS `multiline: true` twin).
        ("input_multiline".to_string(), vec![TY_OPAQUE, TY_BOOL], TY_NIL, false),
        // the font family (the JS `Input().fontFamily(...)` twin — the code
        // editor pins `"monospace"` so caret math sees uniform glyphs).
        ("input_font_family".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("input_qkey".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("input_build".to_string(), vec![TY_OPAQUE], TY_OPAQUE, false),
        // controllers (realm-minted, method rows)
        ("tctrl_new".to_string(), vec![], TY_OPAQUE, false),
        ("undo_new".to_string(), vec![], TY_OPAQUE, false),
        ("tctrl_text".to_string(), vec![TY_OPAQUE], TY_STR, false),
        ("tctrl_set_text".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("tctrl_push_span".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("tctrl_delete_selection".to_string(), vec![TY_OPAQUE], TY_NIL, false),
        ("tctrl_insert_text".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("tctrl_cursor".to_string(), vec![TY_OPAQUE], TY_U64, false),
        ("tctrl_select".to_string(), vec![TY_OPAQUE, TY_U64, TY_U64], TY_NIL, false),
        ("tctrl_clear".to_string(), vec![TY_OPAQUE], TY_NIL, false),
        ("tctrl_paste".to_string(), vec![TY_OPAQUE, TY_STR], TY_NIL, false),
        ("undo_can_undo".to_string(), vec![TY_OPAQUE], TY_BOOL, false),
        ("undo_can_redo".to_string(), vec![TY_OPAQUE], TY_BOOL, false),
        ("undo_clear".to_string(), vec![TY_OPAQUE], TY_NIL, false),
    ]);
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    // span flags (bitfield)
    cx.consts.extend(vec![c("SPAN_ITALIC", 1), c("SPAN_UNDERLINE", 2)]);
}

use rut_core::types::{TY_BOOL, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The opaque wrapper over a shared `TextEditingController`.
pub struct RutTextCtrl(pub Rc<RefCell<TextEditingController>>);

/// The opaque wrapper over a shared `UndoController`.
pub struct RutUndoCtrl(pub Rc<RefCell<UndoController>>);

/// A rich-text span list under construction.
pub(crate) struct RutSpans(pub Vec<SpanData>);

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

/// Install the text-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // Own the handle up front — the `move` row closures below clone from
    // the owned `Rc` (a captured reference would escape this body).
    let handles: Rc<RutHandles> = handles.clone();
    // NOTE for the controller-mutating rows below (`tctrl_set_text` & co):
    // after the write they refresh the MOUNTED editable — mark its node
    // dirty (the controller is an opaque `Rc` binding invisible to the
    // reactive build dedup; without the mark a programmatic write never
    // revisits the element) and request a frame so an idle worker re-arms.
    // The paste path's law (`tree.mark_dirty(focused_id)`), reached from
    // the rut side.
    // ---- text family ----------------------------------------------------
    rut_vm::pkg_fn!(pkg, "text_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, TextView {
            text: None,
            font_size: None,
            font_weight: None,
            color: None,
            spans: None,
            query_key: None,
            on_selection_change: None,
            selectable: false,
            max_lines: None,
            overflow: None,
        })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "text_text", (Opaque<TextView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, v: &str| {
        b.with_mut(vm, |_vm, t| t.text = Some(Val::Static(v.to_string())))
    });
    // Bind to a str SOURCE atom — re-renders when the atom changes.
    rut_vm::pkg_fn!(pkg, "text_bind", (Opaque<TextView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, atom: u64| {
        b.with_mut(vm, |_vm, t| {
            t.text = Some(Val::Reactive(Readable::Source(
                Source::<String>::from_id(AtomId(atom as u32)),
            )));
        })
    });
    // Bind to a DERIVED str atom (the `rs_derive` consumer twin).
    rut_vm::pkg_fn!(pkg, "text_bind_derived", (Opaque<TextView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, derived: u64| {
        b.with_mut(vm, |_vm, t| {
            t.text = Some(Val::Reactive(Readable::Derived(
                Derived::<String>::from_id(AtomId(derived as u32)),
            )));
        })
    });
    rut_vm::pkg_fn!(pkg, "text_size", (Opaque<TextView>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, v: f64| {
        b.with_mut(vm, |_vm, t| t.font_size = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "text_weight", (Opaque<TextView>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, v: f64| {
        b.with_mut(vm, |_vm, t| t.font_weight = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "text_color", (Opaque<TextView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, v: u64| {
        b.with_mut(vm, |_vm, t| t.color = Some(Val::Static(color_of(v))))
    });
    rut_vm::pkg_fn!(pkg, "text_max_lines", (Opaque<TextView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, v: u64| {
        b.with_mut(vm, |_vm, t| t.max_lines = Some(Val::Static(v as u32)))
    });
    rut_vm::pkg_fn!(pkg, "text_clip", (Opaque<TextView>,) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>| {
        b.with_mut(vm, |_vm, t| t.overflow = Some(Val::Static(TextOverflow::Clip)))
    });
    rut_vm::pkg_fn!(pkg, "text_ellipsis", (Opaque<TextView>,) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>| {
        b.with_mut(vm, |_vm, t| t.overflow = Some(Val::Static(TextOverflow::Ellipsis)))
    });
    rut_vm::pkg_fn!(pkg, "text_overflow_visible", (Opaque<TextView>,) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>| {
        b.with_mut(vm, |_vm, t| t.overflow = Some(Val::Static(TextOverflow::Visible)))
    });
    rut_vm::pkg_fn!(pkg, "text_selectable", (Opaque<TextView>, bool) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, v: bool| {
        b.with_mut(vm, |_vm, t| t.selectable = v)
    });
    rut_vm::pkg_fn!(pkg, "text_qkey", (Opaque<TextView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, t| t.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "text_build", (Opaque<TextView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>| {
        let view = b.with(|t| Rc::new(t.clone()) as Rc<dyn crate::core::view::View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- rich-text spans --------------------------------------------------
    rut_vm::pkg_fn!(pkg, "spans_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, RutSpans(Vec::new()))?.handle().clone())
    });
    // span_add(list, text, size, color, weight, flags) — 0 inherits.
    rut_vm::pkg_fn!(pkg, "span_add", (Opaque<RutSpans>, &str, f64, u64, f64, u64) -> (), |vm: &mut rut_vm::interp::Vm, list: Opaque<RutSpans>, text: &str, size: f64, color: u64, weight: f64, flags: u64| {
        list.with_mut(vm, |_vm, list| {
            list.0.push(SpanData {
                text: text.to_string(),
                weight: (weight > 0.0).then_some(weight),
                italic: flags & 1 != 0,
                underline: flags & 2 != 0,
                font_size: (size > 0.0).then_some(size),
                color: (color != 0).then(|| color_of(color)),
            });
        })
    });
    // text_spans(text_spec, spans) — bind the span run to a text spec.
    rut_vm::pkg_fn!(pkg, "text_spans", (Opaque<TextView>, Opaque<RutSpans>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TextView>, spans: Opaque<RutSpans>| {
        let spans = spans.with(|s| s.0.clone())?;
        b.with_mut(vm, |_vm, t| t.spans = Some(spans))
    });

    // ---- input family -------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "input_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, InputView::empty_rut())?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "input_controller", (Opaque<InputView>, Opaque<RutTextCtrl>) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, ctrl: Opaque<RutTextCtrl>| {
        let shared = ctrl.with(|c| c.0.clone())?;
        b.with_mut(vm, |_vm, s| s.set_controller(shared))
    });
    rut_vm::pkg_fn!(pkg, "input_undo", (Opaque<InputView>, Opaque<RutUndoCtrl>) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, undo: Opaque<RutUndoCtrl>| {
        let shared = undo.with(|u| u.0.clone())?;
        b.with_mut(vm, |_vm, s| s.set_undo(shared))
    });
    rut_vm::pkg_fn!(pkg, "input_size", (Opaque<InputView>, f64, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, w: f64, h: f64| {
        b.with_mut(vm, |_vm, s| {
            if w > 0.0 {
                s.set_width(w);
            }
            if h > 0.0 {
                s.set_height(h);
            }
        })
    });
    rut_vm::pkg_fn!(pkg, "input_placeholder", (Opaque<InputView>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, text: &str| {
        b.with_mut(vm, |_vm, s| s.set_placeholder_str(text.to_string()))
    });
    rut_vm::pkg_fn!(pkg, "input_color", (Opaque<InputView>, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, packed: u64| {
        let c = color_of(packed);
        b.with_mut(vm, |_vm, s| s.set_color(c))
    });
    rut_vm::pkg_fn!(pkg, "input_placeholder_color", (Opaque<InputView>, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, packed: u64| {
        let c = color_of(packed);
        b.with_mut(vm, |_vm, s| s.set_placeholder_color(c))
    });
    rut_vm::pkg_fn!(pkg, "input_font_size", (Opaque<InputView>, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.set_font_size(v))
    });
    // input_obscure(builder, on) — the password toggle (the JS
    // `obscureText: true` twin).
    rut_vm::pkg_fn!(pkg, "input_obscure", (Opaque<InputView>, bool) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, on: bool| {
        b.with_mut(vm, |_vm, s| s.set_obscure(on))
    });
    // input_obscure_char(builder, ch) — the configurable obscuring
    // character (the JS `obscuringCharacter: '*'` twin; a multi-char str
    // takes its first char, matching the engine's per-char mask).
    rut_vm::pkg_fn!(pkg, "input_obscure_char", (Opaque<InputView>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, ch: &str| {
        let ch = ch.chars().next().unwrap_or('\u{2022}').to_string();
        b.with_mut(vm, |_vm, s| s.set_obscuring_character_str(ch))
    });
    // input_font_family(builder, family) — the JS `fontFamily(...)` twin
    // (parley generic families: "monospace" / "serif" / sans-serif default).
    rut_vm::pkg_fn!(pkg, "input_font_family", (Opaque<InputView>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, family: &str| {
        b.with_mut(vm, |_vm, s| s.set_font_family_str(family.to_string()))
    });
    // input_multiline(builder, on) — the JS `multiline: true` twin.
    rut_vm::pkg_fn!(pkg, "input_multiline", (Opaque<InputView>, bool) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, on: bool| {
        b.with_mut(vm, |_vm, s| s.set_multiline(on))
    });
    rut_vm::pkg_fn!(pkg, "input_qkey", (Opaque<InputView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.set_query_key(key))
    });
    rut_vm::pkg_fn!(pkg, "input_build", (Opaque<InputView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<InputView>| {
        let view = b.with(|s| Rc::new(s.clone()) as Rc<dyn crate::core::view::View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- controllers: mint + method rows (downcast without a realm borrow)
    rut_vm::pkg_fn!(pkg, "tctrl_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let ctrl = Rc::new(RefCell::new(TextEditingController::new()));
        Ok(Opaque::alloc(vm, RutTextCtrl(ctrl))?.handle().clone())
    });

    rut_vm::pkg_fn!(pkg, "undo_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let undo = Rc::new(RefCell::new(UndoController::new()));
        Ok(Opaque::alloc(vm, RutUndoCtrl(undo))?.handle().clone())
    });

    rut_vm::pkg_fn!(pkg, "tctrl_text", (Opaque<RutTextCtrl>,) -> String, move |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        c.with(|c| Ok(c.0.borrow().text()))?
    });
    let set_handles = handles.clone();
    rut_vm::pkg_fn!(pkg, "tctrl_set_text", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        let handles = set_handles.clone();
        c.with_mut(vm, |_vm, c| {
            let mut ctrl = c.0.borrow_mut();
            ctrl.set_spans(vec![plain_span(text)]);
            if let Some(id) = ctrl.mounted_view() {
                handles.element_tree.mark_dirty(id);
            }
        })?;
        (handles.request_frame)();
        Ok(())
    });
    // Delete the selection (or nothing at an empty caret) — the undo
    // fixture's delete step.
    let del_handles = handles.clone();
    rut_vm::pkg_fn!(pkg, "tctrl_delete_selection", (Opaque<RutTextCtrl>,) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>| {
        let handles = del_handles.clone();
        c.with_mut(vm, |_vm, c| {
            let mut ctrl = c.0.borrow_mut();
            ctrl.delete_selection();
            if let Some(id) = ctrl.mounted_view() {
                handles.element_tree.mark_dirty(id);
            }
        })?;
        (handles.request_frame)();
        Ok(())
    });
    // Insert text at the caret (no selection replace) — the undo fixture's
    // typed-insert step.
    let ins_handles = handles.clone();
    rut_vm::pkg_fn!(pkg, "tctrl_insert_text", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        let handles = ins_handles.clone();
        c.with_mut(vm, |_vm, c| {
            let mut ctrl = c.0.borrow_mut();
            let pos = ctrl.cursor_position();
            ctrl.insert_str_at(pos, text);
            ctrl.set_cursor_position(pos + text.len());
            if let Some(id) = ctrl.mounted_view() {
                handles.element_tree.mark_dirty(id);
            }
        })?;
        (handles.request_frame)();
        Ok(())
    });

    // Append one plain span (the huge-document fixture's authoring loop).
    let push_handles = handles.clone();
    rut_vm::pkg_fn!(pkg, "tctrl_push_span", (Opaque<RutTextCtrl>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, c: Opaque<RutTextCtrl>, text: &str| {
        let handles = push_handles.clone();
        c.with_mut(vm, |_vm, c| {
            let mut spans = c.0.borrow().spans().to_vec();
            spans.push(plain_span(text));
            let mut ctrl = c.0.borrow_mut();
            ctrl.set_spans_preserve_cursor(spans);
            if let Some(id) = ctrl.mounted_view() {
                handles.element_tree.mark_dirty(id);
            }
        })?;
        (handles.request_frame)();
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
