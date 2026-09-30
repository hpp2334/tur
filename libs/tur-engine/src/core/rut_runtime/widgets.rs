//! Phase-4 corpus rows — the builder surface the rut corpus needs:
//! flex alignment / main-axis size, stack alignment / fit, a generic
//! query-key setter, the text style builder, and rich-text spans.
//!
//! Everything here follows the C3 convention: builder opaques + setter
//! rows + `u64` flag consts standing in for engine enums (rut enums do
//! not cross the host boundary).

use std::rc::Rc;

use num_traits::FromPrimitive;

use crate::builtin_plugins::text::controller::SpanData;
use crate::builtin_plugins::text::elements::paragraph::TextOverflow;
use crate::builtin_plugins::text::TextView;
use crate::core::layout::{CrossAxisAlignment, MainAxisAlignment, MainAxisSize, StackFit};
use crate::core::view::Val;

use rut_vm::Opaque;

use super::color_of;
use super::{RutView, ViewBuilder};

/// Declare the corpus rows on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        // flex surface
        ("flex_main_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("flex_cross_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("flex_main_size", vec![TY_OPAQUE, TY_U64], TY_NIL),
        // stack surface
        ("stack_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("stack_fit", vec![TY_OPAQUE, TY_U64], TY_NIL),
        // generic query key
        ("el_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        ("el_vqkey", vec![TY_OPAQUE, TY_STR], TY_OPAQUE),
        // text style builder
        ("el_text_new", vec![TY_STR], TY_OPAQUE),
        ("el_text_bound_new", vec![TY_U64], TY_OPAQUE),
        ("el_text_bound_d_new", vec![TY_U64], TY_OPAQUE),
        ("text_size", vec![TY_OPAQUE, TY_F64], TY_NIL),
        ("text_weight", vec![TY_OPAQUE, TY_F64], TY_NIL),
        ("text_color", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("text_max_lines", vec![TY_OPAQUE, TY_U64], TY_NIL),
        ("text_clip", vec![TY_OPAQUE], TY_NIL),
        ("text_ellipsis", vec![TY_OPAQUE], TY_NIL),
        ("text_overflow_visible", vec![TY_OPAQUE], TY_NIL),
        ("text_selectable", vec![TY_OPAQUE, TY_BOOL], TY_NIL),
        // rich-text spans
        ("spans_new", vec![], TY_OPAQUE),
        (
            "span_add",
            vec![TY_OPAQUE, TY_STR, TY_F64, TY_U64, TY_F64, TY_U64],
            TY_NIL,
        ),
        ("text_spans", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        // stateful cells
        ("mem_new", vec![TY_F64], TY_OPAQUE),
        ("mem_get", vec![TY_OPAQUE], TY_F64),
        ("mem_set", vec![TY_OPAQUE, TY_F64], TY_NIL),
        ("str_parse_f64", vec![TY_STR], TY_F64),
        ("stf_put", vec![TY_U64, TY_F64], TY_NIL),
        ("stf_take", vec![TY_U64], TY_F64),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// The flag consts (u64) for the corpus rows — `FromPrimitive` order of
/// the engine enums they stand in for.
pub fn decl_consts() -> Vec<(String, rut_core::types::TypeId, u64)> {
    use rut_core::types::TY_U64;
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    vec![
        c("MAIN_ALIGN_START", 0),
        c("MAIN_ALIGN_CENTER", 1),
        c("MAIN_ALIGN_END", 2),
        c("MAIN_ALIGN_SPACE_BETWEEN", 3),
        c("MAIN_ALIGN_SPACE_AROUND", 4),
        c("MAIN_ALIGN_SPACE_EVENLY", 5),
        c("CROSS_ALIGN_START", 0),
        c("CROSS_ALIGN_CENTER", 1),
        c("CROSS_ALIGN_END", 2),
        c("CROSS_ALIGN_STRETCH", 3),
        c("MAIN_SIZE_MAX", 0),
        c("MAIN_SIZE_MIN", 1),
        c("STACK_FIT_LOOSE", 0),
        c("STACK_FIT_EXPAND", 1),
        c("STACK_FIT_PASSTHROUGH", 2),
        // span flags (bitfield)
        c("SPAN_ITALIC", 1),
        c("SPAN_UNDERLINE", 2),
    ]
}

fn main_alignment_of(v: u64) -> MainAxisAlignment {
    MainAxisAlignment::from_u64(v).unwrap_or(MainAxisAlignment::Start)
}

fn cross_alignment_of(v: u64) -> CrossAxisAlignment {
    CrossAxisAlignment::from_u64(v).unwrap_or(CrossAxisAlignment::Start)
}

fn main_axis_size_of(v: u64) -> MainAxisSize {
    MainAxisSize::from_u64(v).unwrap_or(MainAxisSize::Max)
}

fn stack_fit_of(v: u64) -> StackFit {
    StackFit::from_u64(v).unwrap_or(StackFit::Loose)
}

/// A mutable f64 cell — the stateful-entry scratch crossing (the stash
/// holds opaques only).
pub(crate) struct RutCell(pub std::cell::Cell<f64>);

/// A rich-text span list under construction.
pub(crate) struct RutSpans(pub Vec<SpanData>);

/// A materialized view wrapped with a query-key override (applied after
/// the inner build, so rows with hardcoded keys can be re-keyed).
pub(crate) struct KeyedView {
    pub inner: Rc<dyn crate::core::view::View>,
    pub key: Vec<String>,
}

impl crate::core::view::View for KeyedView {
    fn build(
        &self,
        cx: &mut dyn crate::core::view::ViewCx,
        parent: crate::core::element::NodeId,
    ) -> crate::core::element::NodeId {
        let id = self.inner.build(cx, parent);
        cx.set_query_key(crate::core::element::ElementNodeId::new(id.as_u64()), self.key.clone());
        id
    }
}

/// Install the corpus-row bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<super::RutHandles>) {
    // ---- flex surface -----------------------------------------------------
    rut_vm::pkg_fn!(pkg, "flex_main_align", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_flex(b, vm, |f| if let ViewBuilder::Flex { main_alignment, .. } = f {
            *main_alignment = Some(main_alignment_of(v));
        })
    });
    rut_vm::pkg_fn!(pkg, "flex_cross_align", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_flex(b, vm, |f| if let ViewBuilder::Flex { cross_alignment, .. } = f {
            *cross_alignment = Some(cross_alignment_of(v));
        })
    });
    rut_vm::pkg_fn!(pkg, "flex_main_size", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_flex(b, vm, |f| if let ViewBuilder::Flex { main_axis_size, .. } = f {
            *main_axis_size = Some(main_axis_size_of(v));
        })
    });

    // ---- stack surface ----------------------------------------------------
    rut_vm::pkg_fn!(pkg, "stack_align", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_builder(b, vm, |b| match b {
            ViewBuilder::Stack { alignment, .. } => {
                *alignment = Some(super::container::alignment_of(v));
                Ok(())
            }
            _ => Err(trap("stack_align on a non-stack builder")),
        })
    });
    rut_vm::pkg_fn!(pkg, "stack_fit", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_builder(b, vm, |b| match b {
            ViewBuilder::Stack { fit, .. } => {
                *fit = Some(stack_fit_of(v));
                Ok(())
            }
            _ => Err(trap("stack_fit on a non-stack builder")),
        })
    });

    // ---- generic query key ------------------------------------------------
    rut_vm::pkg_fn!(pkg, "el_qkey", (Opaque<ViewBuilder>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, key: &str| {
        let key = key.split('/').map(str::to_string).collect::<Vec<_>>();
        b.with_mut(vm, |_vm, b| b.set_query_key(key.clone()))
    });
    // Post-build query-key override: wraps a materialized view and re-keys
    // its node after the inner build (the `queryKey` prop twin for rows
    // that hardcode their key, e.g. the Input / lazy / scroll views).
    rut_vm::pkg_fn!(pkg, "el_vqkey", (Opaque<RutView>, &str) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, v: Opaque<RutView>, key: &str| {
        let inner = v.with(|v| v.0.clone())?;
        let view = Rc::new(KeyedView {
            inner,
            key: key.split('/').map(str::to_string).collect(),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- text style builder ----------------------------------------------
    let empty_text = || TextView {
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
    };
    rut_vm::pkg_fn!(pkg, "el_text_new", (&str,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, content: &str| {
        let mut tv = empty_text();
        tv.text = Some(Val::Static(content.to_string()));
        Ok(Opaque::alloc(vm, ViewBuilder::Text(Box::new(tv)))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_text_bound_new", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let mut tv = empty_text();
        tv.text = Some(Val::Reactive(crate::core::edgy::reactive::Readable::Source(
            crate::core::edgy::reactive::Source::<String>::from_id(
                crate::core::edgy::reactive::AtomId(atom as u32),
            ),
        )));
        Ok(Opaque::alloc(vm, ViewBuilder::Text(Box::new(tv)))?.handle().clone())
    });
    // Bound to a DERIVED str atom (the builder variant of `el_text_bound_d`
    // — style rows + query keys apply).
    rut_vm::pkg_fn!(pkg, "el_text_bound_d_new", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, derived: u64| {
        let mut tv = empty_text();
        tv.text = Some(Val::Reactive(crate::core::edgy::reactive::Readable::Derived(
            crate::core::edgy::reactive::Derived::<String>::from_id(
                crate::core::edgy::reactive::AtomId(derived as u32),
            ),
        )));
        Ok(Opaque::alloc(vm, ViewBuilder::Text(Box::new(tv)))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "text_size", (Opaque<ViewBuilder>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: f64| {
        with_text(b, vm, |t| t.font_size = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "text_weight", (Opaque<ViewBuilder>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: f64| {
        with_text(b, vm, |t| t.font_weight = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "text_color", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_text(b, vm, |t| t.color = Some(Val::Static(color_of(v))))
    });
    rut_vm::pkg_fn!(pkg, "text_max_lines", (Opaque<ViewBuilder>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: u64| {
        with_text(b, vm, |t| t.max_lines = Some(Val::Static(v as u32)))
    });
    rut_vm::pkg_fn!(pkg, "text_clip", (Opaque<ViewBuilder>,) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>| {
        with_text(b, vm, |t| t.overflow = Some(Val::Static(TextOverflow::Clip)))
    });
    rut_vm::pkg_fn!(pkg, "text_ellipsis", (Opaque<ViewBuilder>,) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>| {
        with_text(b, vm, |t| t.overflow = Some(Val::Static(TextOverflow::Ellipsis)))
    });
    rut_vm::pkg_fn!(pkg, "text_overflow_visible", (Opaque<ViewBuilder>,) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>| {
        with_text(b, vm, |t| t.overflow = Some(Val::Static(TextOverflow::Visible)))
    });
    rut_vm::pkg_fn!(pkg, "text_selectable", (Opaque<ViewBuilder>, bool) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, v: bool| {
        with_text(b, vm, |t| t.selectable = v)
    });

    // ---- stateful cells ---------------------------------------------------
    // The opaque stash holds OPQUES only, so stateful entries keep their
    // scratch numbers in f64 cells (`mem_*`) — minted at start, stashed,
    // read/written in the intent entries.
    rut_vm::pkg_fn!(pkg, "mem_new", (f64,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, v: f64| {
        Ok(Opaque::alloc(vm, RutCell(std::cell::Cell::new(v)))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "mem_get", (Opaque<RutCell>,) -> f64, |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutCell>| {
        c.with(|c| c.0.get())
    });
    rut_vm::pkg_fn!(pkg, "mem_set", (Opaque<RutCell>, f64) -> (), |_vm: &mut rut_vm::interp::Vm, c: Opaque<RutCell>, v: f64| {
        c.with_mut(_vm, |_vm, c: &mut RutCell| c.0.set(v))?;
        Ok(())
    });
    // str -> f64 parse (0 on failure) — the edit-field confirm path.
    rut_vm::pkg_fn!(pkg, "str_parse_f64", (&str,) -> f64, |_vm: &mut rut_vm::interp::Vm, s: &str| {
        Ok(s.trim().parse::<f64>().unwrap_or(0.0))
    });
    // Scalar stash slots (atom ids / counts cross entries and async frames
    // as f64 — the opaque stash cannot hold raw numbers).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "stf_put", (u64, f64) -> (), move |_vm: &mut rut_vm::interp::Vm, key: u64, v: f64| {
        h.stash_num.borrow_mut().insert(key, v);
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "stf_take", (u64,) -> f64, move |_vm: &mut rut_vm::interp::Vm, key: u64| {
        Ok(h.stash_num.borrow_mut().remove(&key).unwrap_or(0.0))
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
    // text_spans(text_builder, spans) — bind the span run to a text builder.
    rut_vm::pkg_fn!(pkg, "text_spans", (Opaque<ViewBuilder>, Opaque<RutSpans>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, spans: Opaque<RutSpans>| {
        let spans = spans.with(|s| s.0.clone())?;
        with_text(b, vm, |t| t.spans = Some(spans))
    });
}

fn trap(msg: &str) -> rut_vm::Trap {
    rut_vm::Trap::new(rut_vm::TrapKind::Invalid, msg.to_string())
}

/// Mutate the flex payload of a builder.
fn with_flex(
    b: Opaque<ViewBuilder>,
    vm: &mut rut_vm::interp::Vm,
    f: impl FnOnce(&mut ViewBuilder),
) -> Result<(), rut_vm::Trap> {
    b.with_mut(vm, |_vm, b| match b {
        ViewBuilder::Flex { .. } => {
            f(b);
            Ok(())
        }
        _ => Err(trap("flex_* setter on a non-flex builder")),
    })?
}

/// Mutate the text payload of a builder.
fn with_text(
    b: Opaque<ViewBuilder>,
    vm: &mut rut_vm::interp::Vm,
    f: impl FnOnce(&mut crate::builtin_plugins::text::TextView),
) -> Result<(), rut_vm::Trap> {
    with_builder(b, vm, |b| match b {
        ViewBuilder::Text(tv) => {
            f(tv);
            Ok(())
        }
        _ => Err(trap("text_* setter on a non-text builder")),
    })
}

/// Generic builder access.
fn with_builder(
    b: Opaque<ViewBuilder>,
    vm: &mut rut_vm::interp::Vm,
    f: impl FnOnce(&mut ViewBuilder) -> Result<(), rut_vm::Trap>,
) -> Result<(), rut_vm::Trap> {
    b.with_mut(vm, |_vm, b| f(b))?
}
