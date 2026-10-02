//! The layout families' `tur` host-pkg rows (via the pkg-extension seam):
//! flex (Column/Row), stack, box (Container), sized box, positioned,
//! flexible items (Expanded/Flexible), grid, and the reactive-rows table.
//!
//! Every family is complete unto itself — its own spec opaque, constructor
//! row, setter rows, `*_child` / `*_qkey`, and its own `*_build` terminal
//! (the animation family's shape). Specs are cheap-clone bundles: `build`
//! clones the pieces out and materializes the view, so a spec stays valid
//! (the JS-era "build twice is allowed" semantics). Flags cross as `u64`
//! consts (rut enums do not cross the host boundary).

use std::rc::Rc;

use num_traits::FromPrimitive;

use crate::builtin_plugins::layout::composited_transform::link::CompositedLinkState;
use crate::builtin_plugins::layout::{
    ContainerView, FlexView, FlexibleView, GridView, PositionedView, StackView,
    TableColumnDef, TableView,
};
use crate::builtin_plugins::lazy_container::item_builder::RutEntryBuilder;
use crate::core::edgy::reactive::{AtomId, AnyReadable, Readable, Source};
use crate::core::layout::{
    Alignment, Axis, BorderPosition, ClipBehavior, CrossAxisAlignment, FlexFit,
    MainAxisAlignment, MainAxisSize, StackFit,
};
use crate::core::render::brush::Brush;
use crate::core::rut_runtime::{RutHandles, RutView, color_of, readable_of};
use crate::core::view::{Val, View};

use rut_vm::Opaque;

/// The pkg-extension payload: decl rows at compile time, bodies at boot.
pub(crate) fn install_decl_ext(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    install_decl(cx);
    let Some(handles) = cx.handles else {
        return; // the compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    install(&mut *cx.pkg, &handles);
}

/// Declare the layout rows + flag consts (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        // flex (Column / Row)
        row("flex_column", vec![], TY_OPAQUE),
        row("flex_row", vec![], TY_OPAQUE),
        row("flex_main_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("flex_cross_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("flex_main_size", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("flex_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("flex_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("flex_build", vec![TY_OPAQUE], TY_OPAQUE),
        // stack
        row("stack_new", vec![], TY_OPAQUE),
        row("stack_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("stack_fit", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("stack_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("stack_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("stack_build", vec![TY_OPAQUE], TY_OPAQUE),
        // box (the C3 container surface)
        row("box_new", vec![], TY_OPAQUE),
        row("box_padding", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("box_color", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("box_size", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        row("box_width", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("box_height", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("box_border", vec![TY_OPAQUE, TY_U64, TY_F64, TY_U64], TY_NIL),
        row("box_radius", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("box_shadow", vec![TY_OPAQUE, TY_U64, TY_F64, TY_F64, TY_F64], TY_NIL),
        row("box_align", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("box_clip", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("box_width_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("box_height_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("box_color_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("box_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("box_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("box_build", vec![TY_OPAQUE], TY_OPAQUE),
        // sized box
        row("sizedbox_new", vec![TY_F64, TY_F64], TY_OPAQUE),
        row("sizedbox_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("sizedbox_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("sizedbox_build", vec![TY_OPAQUE], TY_OPAQUE),
        // positioned
        row("pos_new", vec![], TY_OPAQUE),
        row("pos_left", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("pos_top", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("pos_right", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("pos_bottom", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("pos_width", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("pos_height", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("pos_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("pos_build", vec![TY_OPAQUE], TY_OPAQUE),
        // flexible items (Expanded / Flexible)
        row("flexi_new", vec![], TY_OPAQUE),
        row("flexi_flex", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("flexi_flex_bound", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("flexi_fit", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("flexi_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("flexi_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("flexi_build", vec![TY_OPAQUE], TY_OPAQUE),
        // grid
        row("grid_new", vec![], TY_OPAQUE),
        row("grid_max_cross", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("grid_aspect", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("grid_spacing", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
        row("grid_main_extent", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("grid_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("grid_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("grid_build", vec![TY_OPAQUE], TY_OPAQUE),
        // table (+ its column-def list)
        row("table_new", vec![], TY_OPAQUE),
        row("table_columns", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("table_rows_atom", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("table_row_builder", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("table_header_builder", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("table_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("table_build", vec![TY_OPAQUE], TY_OPAQUE),
        row("cols_new", vec![], TY_OPAQUE),
        row("col_fixed", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("col_flex", vec![TY_OPAQUE, TY_F64, TY_F64], TY_NIL),
    ]);
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    cx.consts.extend(vec![
        // Alignment (the `FromPrimitive` order)
        c("ALIGN_TOP_LEFT", 0),
        c("ALIGN_TOP_CENTER", 1),
        c("ALIGN_TOP_RIGHT", 2),
        c("ALIGN_CENTER_LEFT", 3),
        c("ALIGN_CENTER", 4),
        c("ALIGN_CENTER_RIGHT", 5),
        c("ALIGN_BOTTOM_LEFT", 6),
        c("ALIGN_BOTTOM_CENTER", 7),
        c("ALIGN_BOTTOM_RIGHT", 8),
        // ClipBehavior
        c("CLIP_NONE", 0),
        c("CLIP_HARD_EDGE", 1),
        c("CLIP_ANTI_ALIAS", 2),
        // BorderPosition
        c("BORDER_INSIDE", 0),
        c("BORDER_CENTER", 1),
        c("BORDER_OUTSIDE", 2),
        // MainAxisAlignment
        c("MAIN_ALIGN_START", 0),
        c("MAIN_ALIGN_CENTER", 1),
        c("MAIN_ALIGN_END", 2),
        c("MAIN_ALIGN_SPACE_BETWEEN", 3),
        c("MAIN_ALIGN_SPACE_AROUND", 4),
        c("MAIN_ALIGN_SPACE_EVENLY", 5),
        // CrossAxisAlignment
        c("CROSS_ALIGN_START", 0),
        c("CROSS_ALIGN_CENTER", 1),
        c("CROSS_ALIGN_END", 2),
        c("CROSS_ALIGN_STRETCH", 3),
        // MainAxisSize
        c("MAIN_SIZE_MAX", 0),
        c("MAIN_SIZE_MIN", 1),
        // StackFit
        c("STACK_FIT_LOOSE", 0),
        c("STACK_FIT_EXPAND", 1),
        c("STACK_FIT_PASSTHROUGH", 2),
        // FlexFit (Tight, Loose)
        c("FIT_TIGHT", 0),
        c("FIT_LOOSE", 1),
        // Axis (Vertical, Horizontal)
        c("AXIS_VERTICAL", 0),
        c("AXIS_HORIZONTAL", 1),
    ]);
}

use rut_core::types::{TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

// ---------------------------------------------------------------------------
// Family specs — cheap-clone bundles materialized by their `*_build` rows.
// ---------------------------------------------------------------------------

/// The flex (Column / Row) spec.
pub(crate) struct FlexSpec {
    axis: Axis,
    children: Vec<Rc<dyn View>>,
    main_alignment: Option<MainAxisAlignment>,
    cross_alignment: Option<CrossAxisAlignment>,
    main_axis_size: Option<MainAxisSize>,
    query_key: Option<Vec<String>>,
}

/// The stack spec.
pub(crate) struct StackSpec {
    children: Vec<Rc<dyn View>>,
    alignment: Option<Alignment>,
    fit: Option<StackFit>,
    query_key: Option<Vec<String>>,
}

/// The positioned spec (`0 = absent` per anchor — the JS Positioned twins).
pub(crate) struct PositionedSpec {
    left: Option<Val<f64>>,
    top: Option<Val<f64>>,
    right: Option<Val<f64>>,
    bottom: Option<Val<f64>>,
    width: Option<Val<f64>>,
    height: Option<Val<f64>>,
    child: Option<Rc<dyn View>>,
}

/// The flexible-item (Expanded / Flexible) spec.
pub(crate) struct FlexiSpec {
    flex: Option<Val<f64>>,
    fit: FlexFit,
    query_key: Option<Vec<String>>,
    child: Option<Rc<dyn View>>,
}

/// The table spec.
pub(crate) struct TableSpec {
    columns: Vec<TableColumnDef>,
    rows: Option<AnyReadable>,
    build: Option<RutEntryBuilder>,
    build_header: Option<RutEntryBuilder>,
    query_key: Option<Vec<String>>,
}

/// The column-def list opaque (`cols_new` / `col_fixed` / `col_flex`).
pub(crate) struct RutCols(pub(crate) Vec<TableColumnDef>);

/// The composited-link opaque (the shared `CompositedLinkState` Rc).
pub(crate) struct RutLayerLink(pub Rc<CompositedLinkState>);

fn qkey_of(key: &str) -> Vec<String> {
    key.split('/').map(str::to_string).collect()
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

pub(crate) fn alignment_of(v: u64) -> Alignment {
    Alignment::from_u64(v).unwrap_or(Alignment::Center)
}

fn clip_of(v: u64) -> ClipBehavior {
    ClipBehavior::from_u64(v).unwrap_or(ClipBehavior::None)
}

fn border_position_of(v: u64) -> BorderPosition {
    BorderPosition::from_u64(v).unwrap_or(BorderPosition::Inside)
}

/// Install the layout-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // ---- flex (Column / Row) ----------------------------------------------
    rut_vm::pkg_fn!(pkg, "flex_column", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, FlexSpec { axis: Axis::Vertical, children: Vec::new(), main_alignment: None, cross_alignment: None, main_axis_size: None, query_key: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "flex_row", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, FlexSpec { axis: Axis::Horizontal, children: Vec::new(), main_alignment: None, cross_alignment: None, main_axis_size: None, query_key: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "flex_main_align", (Opaque<FlexSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.main_alignment = Some(main_alignment_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "flex_cross_align", (Opaque<FlexSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.cross_alignment = Some(cross_alignment_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "flex_main_size", (Opaque<FlexSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.main_axis_size = Some(main_axis_size_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "flex_child", (Opaque<FlexSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.children.push(child))
    });
    rut_vm::pkg_fn!(pkg, "flex_qkey", (Opaque<FlexSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexSpec>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "flex_build", (Opaque<FlexSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexSpec>| {
        let view = b.with(|s| {
            Rc::new(FlexView {
                direction: Some(s.axis),
                main_alignment: s.main_alignment.map(Val::Static),
                cross_alignment: s.cross_alignment.map(Val::Static),
                main_axis_size: s.main_axis_size.map(Val::Static),
                children: s.children.clone(),
                query_key: s.query_key.clone(),
            }) as Rc<dyn View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- stack --------------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "stack_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, StackSpec { children: Vec::new(), alignment: None, fit: None, query_key: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "stack_align", (Opaque<StackSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<StackSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.alignment = Some(alignment_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "stack_fit", (Opaque<StackSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<StackSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.fit = Some(stack_fit_of(v)))
    });
    rut_vm::pkg_fn!(pkg, "stack_child", (Opaque<StackSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<StackSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.children.push(child))
    });
    rut_vm::pkg_fn!(pkg, "stack_qkey", (Opaque<StackSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<StackSpec>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "stack_build", (Opaque<StackSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<StackSpec>| {
        let view = b.with(|s| {
            Rc::new(StackView {
                fit: s.fit.map(Val::Static),
                alignment: s.alignment.map(Val::Static),
                children: s.children.clone(),
                query_key: s.query_key.clone(),
            }) as Rc<dyn View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- box (the C3 container surface) ------------------------------------
    rut_vm::pkg_fn!(pkg, "box_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ContainerView::default())?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "box_padding", (Opaque<ContainerView>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.padding = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "box_color", (Opaque<ContainerView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, color: u64| {
        b.with_mut(vm, |_vm, s| s.color = Some(Val::Static(Brush::SolidColor(color_of(color)))))
    });
    rut_vm::pkg_fn!(pkg, "box_size", (Opaque<ContainerView>, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, w: f64, h: f64| {
        b.with_mut(vm, |_vm, s| {
            s.width = if w > 0.0 { Some(Val::Static(w)) } else { None };
            s.height = if h > 0.0 { Some(Val::Static(h)) } else { None };
        })
    });
    // box_width / box_height — the single-axis setters. Unlike `box_size`'s
    // builder idiom (0 = unset: `width_height(220, 0)` = fixed width,
    // unconstrained height), these set the axis VERBATIM — 0 is a real
    // zero (the hidden-pane shell: a zero-width container that stays
    // mounted and clips its child away).
    rut_vm::pkg_fn!(pkg, "box_width", (Opaque<ContainerView>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.width = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "box_height", (Opaque<ContainerView>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.height = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "box_border", (Opaque<ContainerView>, u64, f64, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, color: u64, width: f64, position: u64| {
        b.with_mut(vm, |_vm, s| {
            s.border_color = Some(Val::Static(color_of(color)));
            s.border_width = Some(Val::Static(width));
            s.border_position = Some(Val::Static(border_position_of(position)));
        })
    });
    rut_vm::pkg_fn!(pkg, "box_radius", (Opaque<ContainerView>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, r: f64| {
        b.with_mut(vm, |_vm, s| s.border_radius = Some(Val::Static(r)))
    });
    rut_vm::pkg_fn!(pkg, "box_shadow", (Opaque<ContainerView>, u64, f64, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, color: u64, blur: f64, dx: f64, dy: f64| {
        b.with_mut(vm, |_vm, s| {
            s.shadow_color = Some(Val::Static(color_of(color)));
            s.shadow_blur = Some(Val::Static(blur));
            s.shadow_offset = Some((dx, dy));
        })
    });
    rut_vm::pkg_fn!(pkg, "box_align", (Opaque<ContainerView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, a: u64| {
        b.with_mut(vm, |_vm, s| s.alignment = Some(Val::Static(alignment_of(a))))
    });
    rut_vm::pkg_fn!(pkg, "box_clip", (Opaque<ContainerView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, c: u64| {
        b.with_mut(vm, |_vm, s| s.clip_behavior = Some(Val::Static(clip_of(c))))
    });
    rut_vm::pkg_fn!(pkg, "box_width_bound", (Opaque<ContainerView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, w_atom: u64| {
        b.with_mut(vm, |_vm, s| s.width = Some(Val::Reactive(readable_of::<f64>(w_atom))))
    });
    rut_vm::pkg_fn!(pkg, "box_height_bound", (Opaque<ContainerView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, h_atom: u64| {
        b.with_mut(vm, |_vm, s| s.height = Some(Val::Reactive(readable_of::<f64>(h_atom))))
    });
    rut_vm::pkg_fn!(pkg, "box_color_bound", (Opaque<ContainerView>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, color_atom: u64| {
        b.with_mut(vm, |_vm, s| s.color = Some(Val::Reactive(readable_of::<Brush>(color_atom))))
    });
    rut_vm::pkg_fn!(pkg, "box_child", (Opaque<ContainerView>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.children.push(child))
    });
    rut_vm::pkg_fn!(pkg, "box_qkey", (Opaque<ContainerView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "box_build", (Opaque<ContainerView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>| {
        let view = b.with(|s| Rc::new(s.clone()) as Rc<dyn View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- sized box ----------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "sizedbox_new", (f64, f64) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, w: f64, h: f64| {
        let spec = ContainerView {
            width: (w > 0.0).then_some(Val::Static(w)),
            height: (h > 0.0).then_some(Val::Static(h)),
            ..ContainerView::default()
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "sizedbox_child", (Opaque<ContainerView>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.children.push(child))
    });
    rut_vm::pkg_fn!(pkg, "sizedbox_qkey", (Opaque<ContainerView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "sizedbox_build", (Opaque<ContainerView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<ContainerView>| {
        let view = b.with(|s| Rc::new(s.clone()) as Rc<dyn View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- positioned ---------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "pos_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, PositionedSpec { left: None, top: None, right: None, bottom: None, width: None, height: None, child: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "pos_left", (Opaque<PositionedSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.left = (v != 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "pos_top", (Opaque<PositionedSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.top = (v != 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "pos_right", (Opaque<PositionedSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.right = (v != 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "pos_bottom", (Opaque<PositionedSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.bottom = (v != 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "pos_width", (Opaque<PositionedSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.width = (v != 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "pos_height", (Opaque<PositionedSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.height = (v != 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "pos_child", (Opaque<PositionedSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "pos_build", (Opaque<PositionedSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<PositionedSpec>| {
        let view = b.with(|s| {
            Rc::new(PositionedView {
                left: s.left.clone(),
                top: s.top.clone(),
                right: s.right.clone(),
                bottom: s.bottom.clone(),
                width: s.width.clone(),
                height: s.height.clone(),
                child: s.child.clone().expect("pos_build: no child"),
            }) as Rc<dyn View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- flexible items (Expanded / Flexible) -------------------------------
    rut_vm::pkg_fn!(pkg, "flexi_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, FlexiSpec { flex: None, fit: FlexFit::Tight, query_key: None, child: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "flexi_flex", (Opaque<FlexiSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexiSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.flex = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "flexi_flex_bound", (Opaque<FlexiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexiSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| s.flex = Some(Val::Reactive(readable_of::<f64>(atom))))
    });
    rut_vm::pkg_fn!(pkg, "flexi_fit", (Opaque<FlexiSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexiSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.fit = if v == 0 { FlexFit::Tight } else { FlexFit::Loose })
    });
    rut_vm::pkg_fn!(pkg, "flexi_child", (Opaque<FlexiSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexiSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "flexi_qkey", (Opaque<FlexiSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexiSpec>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "flexi_build", (Opaque<FlexiSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<FlexiSpec>| {
        let view = b.with(|s| {
            let view = FlexibleView::new_rut(
                s.flex.clone(),
                s.fit,
                s.child.clone().expect("flexi_build: no child"),
            );
            let mut view = view;
            view.query_key = s.query_key.clone();
            Rc::new(view) as Rc<dyn View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- grid -----------------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "grid_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let spec = GridView {
            max_cross_axis_extent: Val::Static(100.0),
            child_aspect_ratio: None,
            main_axis_extent: None,
            cross_axis_spacing: None,
            main_axis_spacing: None,
            children: Vec::new(),
            query_key: None,
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "grid_max_cross", (Opaque<GridView>, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.max_cross_axis_extent = Val::Static(v))
    });
    rut_vm::pkg_fn!(pkg, "grid_aspect", (Opaque<GridView>, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.child_aspect_ratio = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "grid_spacing", (Opaque<GridView>, f64, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>, cross: f64, main: f64| {
        b.with_mut(vm, |_vm, s| {
            s.cross_axis_spacing = Some(Val::Static(cross));
            s.main_axis_spacing = Some(Val::Static(main));
        })
    });
    rut_vm::pkg_fn!(pkg, "grid_main_extent", (Opaque<GridView>, f64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>, v: f64| {
        b.with_mut(vm, |_vm, s| s.main_axis_extent = Some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "grid_child", (Opaque<GridView>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.children.push(child))
    });
    rut_vm::pkg_fn!(pkg, "grid_qkey", (Opaque<GridView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "grid_build", (Opaque<GridView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<GridView>| {
        let view = b.with(|s| Rc::new(s.clone()) as Rc<dyn View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- table ------------------------------------------------------------
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "table_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        let spec = TableSpec {
            columns: Vec::new(),
            rows: None,
            build: None,
            build_header: None,
            query_key: None,
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "table_columns", (Opaque<TableSpec>, Opaque<RutCols>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TableSpec>, cols: Opaque<RutCols>| {
        let columns = cols.with(|c| c.0.clone())?;
        b.with_mut(vm, |_vm, s| s.columns = columns)
    });
    rut_vm::pkg_fn!(pkg, "table_rows_atom", (Opaque<TableSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TableSpec>, atom: u64| {
        b.with_mut(vm, |_vm, s| {
            s.rows = Some(
                Readable::Source(Source::<crate::core::edgy::Value>::from_id(AtomId(atom as u32)))
                    .to_any(),
            )
        })
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "table_row_builder", (Opaque<TableSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<TableSpec>, cb: &str| {
        let _ = vm;
        let entry = RutEntryBuilder { name: cb.to_string(), face: h.face.clone(), handles: h.clone() };
        b.with_mut(vm, |_vm, s| s.build = Some(entry))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "table_header_builder", (Opaque<TableSpec>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<TableSpec>, cb: &str| {
        let _ = vm;
        let entry = RutEntryBuilder { name: cb.to_string(), face: h.face.clone(), handles: h.clone() };
        b.with_mut(vm, |_vm, s| s.build_header = Some(entry))
    });
    rut_vm::pkg_fn!(pkg, "table_qkey", (Opaque<TableSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<TableSpec>, key: &str| {
        let key = qkey_of(key);
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "table_build", (Opaque<TableSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<TableSpec>| {
        let view = b.with(|s| {
            let build = s.build.clone().expect("table_build: no row builder");
            let mut view = TableView::new_rut(
                s.columns.clone(),
                s.rows.expect("table_build: no rows atom"),
                build,
                s.build_header.clone(),
            );
            view.query_key = s.query_key.clone();
            Rc::new(view) as Rc<dyn View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // column-def list (the table's `columns` crossing)
    rut_vm::pkg_fn!(pkg, "cols_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, RutCols(Vec::new()))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "col_fixed", (Opaque<RutCols>, f64) -> (), |vm: &mut rut_vm::interp::Vm, c: Opaque<RutCols>, w: f64| {
        c.with_mut(vm, |_vm, c| {
            c.0.push(TableColumnDef { width: Some(w), flex: None, min_width: None });
        })
    });
    rut_vm::pkg_fn!(pkg, "col_flex", (Opaque<RutCols>, f64, f64) -> (), |vm: &mut rut_vm::interp::Vm, c: Opaque<RutCols>, flex: f64, min: f64| {
        c.with_mut(vm, |_vm, c| {
            c.0.push(TableColumnDef {
                width: None,
                flex: Some(flex),
                min_width: if min > 0.0 { Some(min) } else { None },
            });
        })
    });
}
