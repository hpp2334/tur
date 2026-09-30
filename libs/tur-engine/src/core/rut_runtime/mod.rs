//! The rut runtime seam — the boa replacement's Phase-1 vertical slice.
//!
//! Architecture: **rut drives, the engine applies.** A loaded rut module's
//! `start()` builds a tree of *pure-Rust view data* through host rows
//! (element builders materialize `Rc<dyn View>` values directly — no boa,
//! no `JsValue` anywhere in the rut-built tree) and stashes the root via
//! `tur::mount`. The engine applies the stashed root into the instance's
//! `ElementTree` right after `start` returns, on the same code path the JS
//! `mount(view)` bridge uses. The rut VM itself is driven by the embedder's
//! pump (`run_ready()` before each flush — never inside a flush iteration,
//! so rut rows never race the flush's boa borrow).
//!
//! Module lifecycle contract (mirrors the JS contract): `entry fn start()`
//! is invoked after boot; `entry fn stop()` — if present — runs (best-effort)
//! before the next load and at destroy; the engine owns root-tree teardown.

use std::rc::Rc;
use std::rc::Weak;

use boa_engine::Context;

use crate::core::app::root::RootView;
use crate::core::app::HostMsg;
use crate::core::edgy::reactive::{AtomId, Readable, Source, ScalarRead};
use crate::core::edgy::value::Value;
use crate::core::js_runtime::TurInstanceContext;
use crate::core::layout::Axis;
use crate::core::layout::{Alignment, CrossAxisAlignment, MainAxisAlignment, MainAxisSize, StackFit};
use crate::core::view::{SharedViewCx, View, ViewFactory, Val};
use crate::builtin_plugins::control_flow::ConditionView;
use crate::builtin_plugins::gesture::PointerInteractView;
use crate::builtin_plugins::layout::{ContainerView, FlexView, FlexibleView, PositionedView, StackView};
use crate::builtin_plugins::text::TextView;
use crate::core::layout::FlexFit;
use crate::core::render::brush::{Brush, Color};
use rut_core::types::{TypeId, TY_BOOL, TY_F64, TY_NIL, TY_OPAQUE, TY_OPT_OPAQUE, TY_STR, TY_U64};
use rut_driver::ModuleBody;
use rut_vm::Opaque;
use rut_vm::interp::{CallArgs, Ret, Vm};
use rut_vm::OpaqueRef;

mod async_caps;
mod collections;
mod composited;
mod container;
mod derive;
mod gesture;
mod image_row;
mod mouse_region;
mod realm;
mod text;
mod virtual_app;
mod widgets;

pub use realm::RutRealm;

/// The `RutView`-opaque → `Rc<dyn View>` crossing (item builders return
/// opaques from `entry fn(index)` calls).
pub fn opaque_to_view(handle: &OpaqueRef) -> Option<Rc<dyn View>> {
    RutRuntime::view_of(handle)
}

/// Rebuild a source handle from a raw atom id — the rut rows' crossing
/// (the ids ARE the atoms). Engine-pub so the pkg extensions (tur-animation's
/// rut rows) can bind reactive props.
pub fn source_of<T>(atom: u64) -> Source<T> {
    Source::from_id(AtomId(atom as u32))
}

/// [`source_of`] as a `Readable`.
pub fn readable_of<T>(atom: u64) -> Readable<T> {
    Readable::Source(source_of(atom))
}

/// The per-instance resource budget. Phase-1 defaults; tunable per embedder.
pub fn default_limits() -> rut_vm::interp::Limits {
    rut_vm::interp::Limits {
        fuel: Some(50_000_000),
        heap_limit_bytes: Some(64 * 1024 * 1024),
        interrupt_every: 1024,
    }
}

// ---------------------------------------------------------------------------
// View payloads — the opaque boxes rut rows mint and pass.
// ---------------------------------------------------------------------------

/// A materialized view (`Rc<dyn View>`) sealed in an opaque box.
pub struct RutView(pub Rc<dyn View>);

/// A native edgy [`Value`] sealed in an opaque box — the rut rail's handle
/// to structured atom values (lists / maps; the Phase-2-B5 round-trip
/// surface; typed rut-side lists/records come with the next phase's
/// breadth).
pub struct RutValue(pub Value);

/// A builder under construction — materialized by `tur::el_build`.
/// (Box-variant is boxed to keep the enum small; setters mutate through it.)
pub(crate) enum ViewBuilder {
    Flex {
        axis: Axis,
        children: Vec<Rc<dyn View>>,
        main_alignment: Option<MainAxisAlignment>,
        cross_alignment: Option<CrossAxisAlignment>,
        main_axis_size: Option<MainAxisSize>,
        query_key: Option<Vec<String>>,
    },
    Stack {
        children: Vec<Rc<dyn View>>,
        alignment: Option<Alignment>,
        fit: Option<StackFit>,
        query_key: Option<Vec<String>>,
    },
    /// The C3 full-surface container: setter rows mutate the spec in
    /// place; `el_build` materializes it.
    Box(Box<ContainerView>),
    /// The Phase-4 text builder: a `TextView` under construction (style
    /// setter rows mutate it; `el_build` materializes it).
    Text(Box<TextView>),
}

impl ViewBuilder {
    fn materialize(self) -> Rc<dyn View> {
        match self {
            ViewBuilder::Flex {
                axis,
                children,
                main_alignment,
                cross_alignment,
                main_axis_size,
                query_key,
            } => Rc::new(FlexView {
                direction: Some(axis),
                main_alignment: main_alignment.map(Val::Static),
                cross_alignment: cross_alignment.map(Val::Static),
                main_axis_size: main_axis_size.map(Val::Static),
                children,
                query_key,
            }),
            ViewBuilder::Stack {
                children,
                alignment,
                fit,
                query_key,
            } => Rc::new(StackView {
                fit: fit.map(Val::Static),
                alignment: alignment.map(Val::Static),
                children,
                query_key,
            }),
            ViewBuilder::Box(spec) => Rc::new(*spec),
            ViewBuilder::Text(tv) => Rc::new(*tv),
        }
    }

    /// Attach a query key to any builder variant (the `el_qkey` row).
    fn set_query_key(&mut self, key: Vec<String>) {
        match self {
            ViewBuilder::Flex { query_key, .. }
            | ViewBuilder::Stack { query_key, .. } => *query_key = Some(key),
            ViewBuilder::Box(box_) => box_.query_key = Some(key),
            ViewBuilder::Text(tv) => tv.query_key = Some(key),
        }
    }
}

/// `0xRRGGBBAA` packed color → engine `Color`.
fn color_of(packed: u64) -> Color {
    Color::rgba(
        ((packed >> 24) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

/// A pre-built branch for `tur::condition` — `create` clones the Rc, so a
/// branch swap needs NO rut invocation during flush (the factory is pure
/// Rust; the subtree was authored at `start` time).
struct PreBuilt(Rc<dyn View>);

impl ViewFactory for PreBuilt {
    fn create(&self, _realm: Option<&mut Context>) -> Option<Rc<dyn View>> {
        Some(self.0.clone())
    }
}

// ---------------------------------------------------------------------------
// The `tur` host package — decl rows (mounted in-memory as a Module) +
// bodies (a HostPkg installed into the per-instance HostRegistry).
// ---------------------------------------------------------------------------

/// The in-memory `tur` host-pkg Module (the DECL side): the surface rut
/// code compiles against. Mounted via `Session::register_module` — no
/// filesystem involved.
pub fn tur_decl_module() -> rut_driver::Module {
    let row = |name: &str, params: Vec<TypeId>, ret: TypeId| (name.to_string(), params, ret, false);
    let mut host_funcs: Vec<(String, Vec<TypeId>, TypeId, bool)> = vec![
        row("el_column", vec![], TY_OPAQUE),
        row("el_row", vec![], TY_OPAQUE),
        row("el_text", vec![TY_STR], TY_OPAQUE),
        row("el_text_bound", vec![TY_U64], TY_OPAQUE),
        row("el_text_styled", vec![TY_STR, TY_F64, TY_U64], TY_OPAQUE),
        row("el_scroll", vec![TY_BOOL, TY_OPAQUE], TY_OPAQUE),
        row("el_scroll_at", vec![TY_F64, TY_BOOL, TY_OPAQUE], TY_OPAQUE),
        row("el_button", vec![TY_U64, TY_U64, TY_STR, TY_STR], TY_OPAQUE),
        row("el_stack", vec![], TY_OPAQUE),
        row("el_box", vec![TY_U64, TY_F64, TY_OPAQUE], TY_OPAQUE),
        row("el_expand", vec![TY_F64, TY_OPAQUE], TY_OPAQUE),
        row("el_positioned", vec![TY_F64, TY_F64, TY_OPAQUE], TY_OPAQUE),
        row("condition", vec![TY_U64, TY_OPAQUE, TY_OPAQUE], TY_OPAQUE),
        row("rs_source_bool", vec![TY_BOOL], TY_U64),
        row("rs_set_bool", vec![TY_U64, TY_BOOL], TY_NIL),
        row("rs_get_bool", vec![TY_U64], TY_BOOL),
        row("el_build", vec![TY_OPAQUE], TY_OPAQUE),
        row("el_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("mount", vec![TY_OPAQUE], TY_NIL),
        row("rs_source_str", vec![TY_STR], TY_U64),
        row("rs_set_str", vec![TY_U64, TY_STR], TY_NIL),
        row("rs_get_str", vec![TY_U64], TY_STR),
        row("rs_source_f64", vec![], TY_U64),
        row("rs_set_f64", vec![TY_U64, TY_F64], TY_NIL),
        row("rs_get_f64", vec![TY_U64], TY_F64),
        // structured values (list/map atoms over the native-KV substrate)
        row("rs_list_new", vec![], TY_OPAQUE),
        row("rs_list_push", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("rs_map_new", vec![], TY_OPAQUE),
        row("rs_map_set", vec![TY_OPAQUE, TY_STR, TY_STR], TY_NIL),
        row("rs_source_value", vec![TY_OPAQUE], TY_U64),
        row("rs_set_value", vec![TY_U64, TY_OPAQUE], TY_NIL),
        row("rs_get_value", vec![TY_U64], TY_OPAQUE),
        row("rs_value_len", vec![TY_OPAQUE], TY_U64),
        row("rs_value_item", vec![TY_OPAQUE, TY_U64], TY_STR),
        row("rs_value_get", vec![TY_OPAQUE, TY_STR], TY_STR),
        // the opaque stash (cross-entry hand-off)
        row("st_put", vec![TY_U64, TY_OPAQUE], TY_NIL),
        row("st_take", vec![TY_U64], TY_OPT_OPAQUE),
    ];
    // C1 — text input rows (realm-minted controllers).
    host_funcs.extend(text::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    // C2 — collections rows (Each over list atoms, lazy containers).
    host_funcs.extend(collections::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    // C3 — the container full surface + flag consts.
    host_funcs.extend(container::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    // C4 — gestures, keyboard, focus (intent records on the drain).
    host_funcs.extend(gesture::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    // C6 — async capabilities (clipboard + bytes helpers; the async rows
    // ride the driver's five-row family expansion).
    host_funcs.extend(async_caps::decl_rows());
    // C7 — lifecycle + virtual apps.
    host_funcs.extend(virtual_app::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    // C8 — derived atoms + watch (the guarded flush-time VM call).
    host_funcs.extend(derive::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    // Phase 4 — the corpus surface: builder breadth (flex/stack/text/qkey,
    // spans), MouseRegion, composited transforms, images.
    host_funcs.extend(widgets::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    host_funcs.extend(mouse_region::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    host_funcs.extend(composited::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    host_funcs.extend(image_row::decl_rows().into_iter().map(|(n, p, r)| (n, p, r, false)));
    let mut consts = container::decl_consts();
    consts.extend(widgets::decl_consts());
    consts.extend(mouse_region::decl_consts());
    consts.extend(image_row::decl_consts());
    rut_driver::Module {
        namespace: Some("tur".to_string()),
        body: ModuleBody::Host {
            host_funcs,
            consts,
            native_types: Vec::new(),
            native_traits: Vec::new(),
            native_fns: Vec::new(),
            native_impls: Vec::new(),
        },
        ..Default::default()
    }
}

/// The bodies. `handles` is the per-instance bridge state the rows close
/// over (the pending root stash; the dispatch registry + reactive rails
/// arrive in later phases).
fn install_tur_pkg(
    hosts: &mut rut_vm::interp::HostRegistry,
    ctx: &rut_vm::interp::HostPkgContext,
    handles: &Rc<RutHandles>,
    exts: &[RutPkgExt],
) {
    let mut pkg = rut_vm::interp::HostPkg::new("tur");

    // mint a flex builder
    rut_vm::pkg_fn!(pkg, "el_column", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Flex { axis: Axis::Vertical, children: Vec::new(), main_alignment: None, cross_alignment: None, main_axis_size: None, query_key: None })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_row", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Flex { axis: Axis::Horizontal, children: Vec::new(), main_alignment: None, cross_alignment: None, main_axis_size: None, query_key: None })?.handle().clone())
    });
    // static text
    rut_vm::pkg_fn!(pkg, "el_text", (&str,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, content: &str| {
        let view = Rc::new(TextView {
            text: Some(Val::Static(content.to_string())),
            font_size: None,
            font_weight: None,
            color: None,
            spans: None,
            query_key: None,
            on_selection_change: None,
            selectable: false,
            max_lines: None,
            overflow: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // builder -> materialized view
    rut_vm::pkg_fn!(pkg, "el_build", (Opaque<ViewBuilder>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>| {
        // `with` lends the payload — swap out a dummy to consume the builder
        let built = b.with_mut(vm, |_vm, b| {
            let dummy = ViewBuilder::Flex { axis: Axis::Vertical, children: Vec::new(), main_alignment: None, cross_alignment: None, main_axis_size: None, query_key: None };
            std::mem::replace(b, dummy).materialize()
        })?;
        Ok(Opaque::alloc(vm, RutView(built))?.handle().clone())
    });
    // attach a materialized child to a flex builder
    rut_vm::pkg_fn!(pkg, "el_child", (Opaque<ViewBuilder>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, child: Opaque<RutView>| {
        let child_view = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, b| {
            match b {
                ViewBuilder::Flex { children, .. } | ViewBuilder::Stack { children, .. } => {
                    children.push(child_view);
                }
                ViewBuilder::Box(spec) => spec.children.push(child_view),
                // A Text builder takes no children.
                ViewBuilder::Text(_) => {}
            }
        })?;
        Ok(())
    });
    // stash the root — the engine applies it after `start` returns. The
    // C8 no-mount law: a face-driven call (a derive / item builder
    // materializing mid-flush) may NOT re-mount — the trap is reported
    // through the error rail and the flush continues.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mount", (Opaque<RutView>,) -> (), move |vm: &mut rut_vm::interp::Vm, view: Opaque<RutView>| {
        if h.face_busy.get() > 0 {
            return Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                "tur::mount inside a face call (derive / item builder) —                  mounting is a start-time or intent-drain-time op only",
            ));
        }
        let root = view.with(|v| v.0.clone())?;
        let _ = vm;
        *h.pending_root.borrow_mut() = Some(root);
        Ok(())
    });

    // ---- reactive rails (Phase 2 — native-KV substrate) -----------------
    //
    // Atoms are the engine's edgy store (`core::edgy`) — the SAME KV the
    // JS realm and the element tree use — addressed by raw `AtomId` as
    // u64. The KV holds native `Value`s, so every row below is
    // realm-free: no JsValue is ever constructed. Writes cross; reads are
    // served by the rut-side mirror (the wrapper atoms hold their current
    // value), so no read path ever needs the boa context. The flush
    // fixed-point (stale atoms → dirty subscribers → re-layout) is
    // entirely the engine's existing machinery: a bound Text re-renders on
    // `rs_set_*` with zero new engine code.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_str", (&str,) -> u64, move |vm: &mut rut_vm::interp::Vm, v: &str| {
        let _ = vm;
        let s: Source<Value> = h.store.bridge().decl_source(Value::str(v));
        Ok(s.id().0 as u64)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_str", (u64, &str) -> (), move |vm: &mut rut_vm::interp::Vm, atom: u64, v: &str| {
        let _ = vm;
        h.store
            .bridge()
            .set_source(Source::<Value>::from_id(AtomId(atom as u32)), Value::str(v))
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_str: {e}")))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_f64", () -> u64, move |vm: &mut rut_vm::interp::Vm| {
        let _ = vm;
        let s: Source<Value> = h.store.bridge().decl_source(Value::Num(0.0));
        Ok(s.id().0 as u64)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_get_f64", (u64,) -> f64, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let _ = vm;
        match h.store.read_value(AtomId(atom as u32)).as_ref().and_then(ScalarRead::from_value) {
            Some(ScalarRead::Num(n)) => Ok(n),
            _ => Ok(0.0),
        }
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_get_str", (u64,) -> String, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let _ = vm;
        // Boa-free: the KV holds native Values, so the read never touches
        // the realm.
        match h.store.read_value(AtomId(atom as u32)).as_ref().and_then(ScalarRead::from_value) {
            Some(ScalarRead::Str(s)) => Ok(s),
            _ => Ok(String::new()),
        }
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_f64", (u64, f64) -> (), move |vm: &mut rut_vm::interp::Vm, atom: u64, v: f64| {
        let _ = vm;
        h.store
            .bridge()
            .set_source(Source::<Value>::from_id(AtomId(atom as u32)), Value::Num(v))
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_f64: {e}")))
    });
    // a Text bound to a str atom — re-renders when the atom changes
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_text_bound", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let _ = &h;
        let view = Rc::new(TextView {
            text: Some(Val::Reactive(Readable::Source(Source::<String>::from_id(AtomId(atom as u32))))),
            font_size: None,
            font_weight: None,
            color: None,
            spans: None,
            query_key: Some(vec!["rut".to_string(), "text".to_string()]),
            on_selection_change: None,
            selectable: false,
            max_lines: None,
            overflow: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- callbacks (Phase 3): the intent-queue rail --------------------
    //
    // Element callbacks are edgy Rust mutations whose closures are
    // boa-free: they push (name, id, payload) onto `pending_calls` and
    // the PUMP drains them into `vm.call(name, (id, payload))` after
    // flush. A callback may mount — `apply_root` runs in the same drain,
    // with the pump's boa borrow. The click payload is a monotonic
    // per-button count (full event payloads arrive with the gesture
    // bridge later in the migration).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_button", (u64, u64, &str, &str) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, id_a: u64, id_b: u64, cb: &str, label: &str| {
        let h2 = h.clone();
        let cb = cb.to_string();
        let dirty = h.dirty.clone();
        let mutation = h.store.bridge().build_mutate(move |_bridge, _args, _boa| {
            let n = h2.click_seq.get() + 1;
            h2.click_seq.set(n);
            h2.pending_calls.borrow_mut().push(Intent::Click { name: cb.clone(), a: id_a, b: id_b, seq: n as f64 });
            dirty.set(true);
            Ok(crate::core::edgy::Value::Nil)
        });
        let view = Rc::new(PointerInteractView {
            behavior: None,
            on_click: Some(crate::core::edgy::mutation::MutationHandle::<
                crate::builtin_plugins::gesture::PointerInteractEvent,
            >::new(mutation)),
            on_pointer_down: None,
            on_pointer_move: None,
            on_pointer_up: None,
            on_context_menu: None,
            query_key: None,
            child: Some(Rc::new(TextView {
                text: Some(Val::Static(label.to_string())),
                font_size: None,
                font_weight: None,
                color: None,
                spans: None,
                query_key: None,
                on_selection_change: None,
                selectable: false,
                max_lines: None,
                overflow: None,
            })),
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // stack builder — same el_child/el_build flow as flex
    rut_vm::pkg_fn!(pkg, "el_stack", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Stack { children: Vec::new(), alignment: None, fit: None, query_key: None })?.handle().clone())
    });
    // a painted box: color + padding around one child
    rut_vm::pkg_fn!(pkg, "el_box", (u64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, color: u64, padding: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(ContainerView {
            width: None,
            height: None,
            padding: Some(Val::Static(padding)),
            color: Some(Val::Static(Brush::SolidColor(color_of(color)))),
            border_color: None,
            border_width: None,
            border_radius: None,
            border_position: None,
            clip_behavior: None,
            shadow_color: None,
            shadow_blur: None,
            alignment: None,
            shadow_offset: None,
            query_key: None,
            children: vec![child],
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // a flex item that fills its slot (Expanded)
    rut_vm::pkg_fn!(pkg, "el_expand", (f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, flex: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(FlexibleView::new_rut(Some(Val::Static(flex)), FlexFit::Tight, child));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // a child anchored inside a Stack
    rut_vm::pkg_fn!(pkg, "el_positioned", (f64, f64, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, left: f64, top: f64, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(PositionedView {
            left: Some(Val::Static(left)),
            top: Some(Val::Static(top)),
            right: None,
            bottom: None,
            width: None,
            height: None,
            child,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // conditional: both branches authored at start time; the swap is pure
    // engine (the factory clones a pre-built Rc — no rut during flush).
    rut_vm::pkg_fn!(pkg, "condition", (u64, Opaque<RutView>, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, when: u64, then_v: Opaque<RutView>, else_v: Opaque<RutView>| {
        let then_v = then_v.with(|v| v.0.clone())?;
        let else_v = else_v.with(|v| v.0.clone())?;
        let view = Rc::new(ConditionView::new_rut(
            Val::Reactive(crate::core::edgy::reactive::Readable::Source(
                crate::core::edgy::reactive::Source::<bool>::from_id(AtomId(when as u32)),
            )),
            Rc::new(PreBuilt(then_v)),
            Rc::new(PreBuilt(else_v)),
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // styled text: explicit font size + packed color (every real UI styles)
    rut_vm::pkg_fn!(pkg, "el_text_styled", (&str, f64, u64) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, content: &str, size: f64, color: u64| {
        let view = Rc::new(TextView {
            text: Some(Val::Static(content.to_string())),
            font_size: Some(Val::Static(size)),
            font_weight: None,
            color: Some(Val::Static(color_of(color))),
            spans: None,
            query_key: None,
            on_selection_change: None,
            selectable: false,
            max_lines: None,
            overflow: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // a scroll viewport (wheel-driven; controller support comes with the
    // controller rows)
    rut_vm::pkg_fn!(pkg, "el_scroll", (bool, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, vertical: bool, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let view = Rc::new(crate::builtin_plugins::scroll::ScrollViewView::new_rut(
            Some(Val::Static(if vertical { Axis::Vertical } else { Axis::Horizontal })),
            child,
        ));
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // scroll viewport with a one-shot initial offset (the JS controller's
    // `initialOffset` twin — applied after the first content layout).
    rut_vm::pkg_fn!(pkg, "el_scroll_at", (f64, bool, Opaque<RutView>) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, initial: f64, vertical: bool, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let mut view = crate::builtin_plugins::scroll::ScrollViewView::new_rut(
            Some(Val::Static(if vertical { Axis::Vertical } else { Axis::Horizontal })),
            child,
        );
        view.initial_offset = Some(Val::Static(initial));
        Ok(Opaque::alloc(vm, RutView(Rc::new(view)))?.handle().clone())
    });

    // bool atoms (the condition rail's driver)
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_bool", (bool,) -> u64, move |vm: &mut rut_vm::interp::Vm, v: bool| {
        let _ = vm;
        let s: Source<Value> = h.store.bridge().decl_source(Value::Bool(v));
        Ok(s.id().0 as u64)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_get_bool", (u64,) -> bool, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let _ = vm;
        match h.store.read_value(AtomId(atom as u32)).as_ref().and_then(ScalarRead::from_value) {
            Some(ScalarRead::Bool(b)) => Ok(b),
            _ => Ok(false),
        }
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_bool", (u64, bool) -> (), move |vm: &mut rut_vm::interp::Vm, atom: u64, v: bool| {
        let _ = vm;
        h.store
            .bridge()
            .set_source(Source::<Value>::from_id(AtomId(atom as u32)), Value::Bool(v))
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_bool: {e}")))
    });

    // ---- structured values (Phase 2-B5) ---------------------------------
    //
    // List / map atoms over the native `Value` KV, addressed through
    // opaque value handles (`RutValue`). This is the minimal round-trip
    // proof the phase asks for — breadth (typed rut lists/maps, records,
    // iteration) is the next phase. Build in-place on an opaque handle,
    // then bind whole values to atoms:
    //
    //   let v = rs_list_new();  rs_list_push(v, "a");
    //   let atom = rs_source_value(v);   rs_set_value(atom, rs_list_new());
    //   let got = rs_get_value(atom);    rs_value_len(got)
    rut_vm::pkg_fn!(pkg, "rs_list_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, RutValue(Value::list(Vec::new())))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "rs_list_push", (Opaque<RutValue>, &str) -> (), move |vm: &mut rut_vm::interp::Vm, v: Opaque<RutValue>, item: &str| {
        v.with_mut(vm, |_vm, v| {
            let items = match &v.0 {
                Value::List(items) => items.as_ref().clone(),
                _ => Vec::new(),
            };
            v.0 = Value::list(items.into_iter().chain(std::iter::once(Value::str(item))));
        })?;
        Ok(())
    });
    rut_vm::pkg_fn!(pkg, "rs_map_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, RutValue(Value::map(Vec::<(Rc<str>, Value)>::new())))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "rs_map_set", (Opaque<RutValue>, &str, &str) -> (), move |vm: &mut rut_vm::interp::Vm, v: Opaque<RutValue>, key: &str, item: &str| {
        v.with_mut(vm, |_vm, v| {
            let mut entries = match &v.0 {
                Value::Map(entries) => entries.as_ref().clone(),
                _ => std::collections::BTreeMap::new(),
            };
            entries.insert(std::rc::Rc::from(key), Value::str(item));
            v.0 = Value::Map(std::rc::Rc::new(entries));
        })?;
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_value", (Opaque<RutValue>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, v: Opaque<RutValue>| {
        let value = v.with(|v| v.0.clone())?;
        let s: Source<Value> = h.store.bridge().decl_source(value);
        Ok(s.id().0 as u64)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_value", (u64, Opaque<RutValue>) -> (), move |_vm: &mut rut_vm::interp::Vm, atom: u64, v: Opaque<RutValue>| {
        let value = v.with(|v| v.0.clone())?;
        h.store
            .bridge()
            .set_source(Source::<Value>::from_id(AtomId(atom as u32)), value)
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_value: {e}")))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_get_value", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        // Boa-free: the native KV serves the fresh slot without the realm.
        let value = h.store.read_value(AtomId(atom as u32)).unwrap_or(Value::Nil);
        Ok(Opaque::alloc(vm, RutValue(value))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "rs_value_len", (Opaque<RutValue>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, v: Opaque<RutValue>| {
        let len = v.with(|v| match &v.0 {
            Value::List(items) => items.len() as u64,
            Value::Map(entries) => entries.len() as u64,
            _ => 0,
        })?;
        Ok(len)
    });
    rut_vm::pkg_fn!(pkg, "rs_value_item", (Opaque<RutValue>, u64) -> String, move |_vm: &mut rut_vm::interp::Vm, v: Opaque<RutValue>, index: u64| {
        let item = v.with(|v| match &v.0 {
            Value::List(items) => items
                .get(index as usize)
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        })?;
        Ok(item.unwrap_or_default())
    });
    rut_vm::pkg_fn!(pkg, "rs_value_get", (Opaque<RutValue>, &str) -> String, move |_vm: &mut rut_vm::interp::Vm, v: Opaque<RutValue>, key: &str| {
        let item = v.with(|v| v.0.get(key).and_then(Value::as_str).map(str::to_string))?;
        Ok(item.unwrap_or_default())
    });

    // C1 — text input (realm-minted controllers, method rows).
    text::install(&mut pkg, handles);
    // C2 — collections (Each + lazy containers).
    collections::install(&mut pkg, handles);
    // C3 — container full surface + SizedBox.
    container::install(&mut pkg, handles);
    // C4 — gestures + keyboard + focus.
    gesture::install(&mut pkg, handles);
    // C6 — async capabilities.
    async_caps::install(&mut pkg, handles);
    // C7 — lifecycle + virtual apps.
    virtual_app::install(&mut pkg, handles);
    // C8 — derived atoms + watch.
    derive::install(&mut pkg, handles);
    // Phase 4 — the corpus surface.
    widgets::install(&mut pkg, handles);
    mouse_region::install(&mut pkg, handles);
    composited::install(&mut pkg, handles);
    image_row::install(&mut pkg, handles);
    // The opaque stash (the cross-entry hand-off rail).
    {
        let h = handles.clone();
        rut_vm::pkg_fn!(pkg, "st_put", (u64, OpaqueRef) -> (), move |_vm: &mut rut_vm::interp::Vm, key: u64, o: OpaqueRef| {
            h.stash.borrow_mut().insert(key, o);
            Ok(())
        });
        let h = handles.clone();
        rut_vm::pkg_fn!(pkg, "st_take", (u64,) -> Option<OpaqueRef>, move |_vm: &mut rut_vm::interp::Vm, key: u64| {
            Ok(h.stash.borrow_mut().remove(&key))
        });
    }
    // Plugin extensions (tur-animation's C5 rows) — AFTER the engine rows,
    // so an extension may lean on them.
    let mut ext_decl = Vec::new();
    let mut ext_consts = Vec::new();
    for ext in exts {
        ext(&mut RutPkgCx {
            decl: &mut ext_decl,
            consts: &mut ext_consts,
            pkg: &mut pkg,
            handles: Some(handles),
        });
    }

    hosts.install_host_pkg(ctx, pkg);
}
// ---------------------------------------------------------------------------
// RutHandles — per-instance bridge state shared with the row closures.
// ---------------------------------------------------------------------------

pub struct RutHandles {
    /// The instance's reactive store — the SAME KV the JS realm and the
    /// element tree share; rut atoms are edgy atoms addressed by raw id.
    pub store: crate::core::edgy::reactive::Store,
    /// The instance's app-dirty flag (rut callbacks raise it when they
    /// stash work so an idle worker wakes).
    pub dirty: Rc<std::cell::Cell<bool>>,
    /// The instance-owned tree handle (a cheap clone of the one the JS
    /// realm shares) — `apply_root` builds into it.
    pub element_tree: crate::core::elements::NodeTree,
    /// The instance's focus manager — the `focus_request` row targets it
    /// (the FocusChange flush pushes the focus/blur mutations next frame).
    pub focus_manager: Rc<std::cell::RefCell<crate::core::focus::FocusManager>>,
    /// The root stashed by `tur::mount` during `start`, applied by the
    /// engine after the call returns (outside the VM, on the mount path).
    pub pending_root: std::cell::RefCell<Option<Rc<dyn View>>>,
    /// Callback intents queued by element callbacks (the rut closure
    /// closures are boa-free: they only push here). Drained at pump level
    /// after flush — `(callback name, id, payload)`.
    pub pending_calls: std::cell::RefCell<Vec<Intent>>,
    /// Monotonic click counter stamped into click intents.
    pub click_seq: std::cell::Cell<u64>,
    /// The module-facing opaque stash — `st_put` / `st_take` let a module
    /// hold host objects across entry calls (an async frame cannot carry
    /// opaque params, so the stash is the hand-off rail).
    pub stash: std::cell::RefCell<std::collections::HashMap<u64, OpaqueRef>>,
    /// The worker→host channel — runtime-error reports for face traps ride
    /// the same `RuntimeError` message the JS rail uses.
    pub host_tx: crate::core::app::HostTx,
    /// The engine's shared clock — the animation rows' `now_ms` source.
    pub clock: Rc<dyn boa_engine::context::time::Clock>,
    /// The engine-wide mutation queue — animation `onTick` callbacks ride
    /// it (same dispatch path the JS controllers use).
    pub mutation_queue: Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>,
    /// The instance context — capability lookups + worker-side spawns (the
    /// async capability rows: clipboard / net / filepicker).
    pub js_ctx: TurInstanceContext,
    /// The realm face (Phase C1): rows that must mint or inspect
    /// JS-class-backed state (a `TextEditingController`, an animation
    /// controller) borrow the realm through it. Detached until boot arms
    /// it; a rut-only module that never calls a realm-demanding row keeps
    /// the realm unallocated.
    pub realm: crate::core::rut_runtime::RutRealm,
    /// The flush-time VM face (Phase C2): view factories / deriveds minted
    /// by rows reach the VM through it. Detached until boot installs the
    /// VM; guards (depth, no-mount) live here.
    pub face: Rc<VmFace>,
    /// Above zero while a face-driven VM call is in flight — `tur::mount`
    /// traps inside one (the C8 no-mount law: a derive/build that tries to
    /// re-mount the tree can never wedge the frame).
    pub face_busy: std::cell::Cell<u32>,
}

/// One queued callback intent — the payload the drain dispatches into an
/// `entry fn`. The legacy click shape (`(name, a, b, seq)`) plus the
/// Phase-C record payloads (keys, pointer positions, raw values) that the
/// gesture / animation / watch rails queue.
#[derive(Clone, Debug)]
pub enum Intent {
    /// `el_button`'s click: `(name, id_a, id_b, seq)` — the Phase-3 shape.
    Click { name: String, a: u64, b: u64, seq: f64 },
    /// A key event from `el_focusable`'s `onKeyDown` mutation.
    Key { name: String, id: u64, key: String, code: String, modifiers: u64, kind: u64 },
    /// A pointer event from `el_gesture`'s down/move/up/context-menu
    /// mutations: `(name, id, local_x, local_y, global_x, global_y, button)`.
    Pointer {
        name: String,
        id: u64,
        lx: f64,
        ly: f64,
        gx: f64,
        gy: f64,
        button: u64,
    },
    /// A raw value payload (animation `onTick(eased)`, `watch(atom, cb)`
    /// change deliveries).
    Value { name: String, a: u64, value: crate::core::edgy::Value },
    /// A bytes payload (net-stream chunks): `entry fn cb(id, data: bytes)`.
    Bytes { name: String, a: u64, data: Vec<u8> },
}

/// The flush-time VM face — view factories / deriveds minted by rows reach
/// the VM through a `Weak` to it, so a module swap (which drops the
/// runtime) detaches every minted face instead of leaking stale frames.
///
/// Guards (the C8 decided law, applied to every face call):
/// - **fuel-capped**: the VM's own budget drives the call; a face call that
///   exhausts it is retried with bounded extra fuel, then bailed (reported,
///   machine returned to idle).
/// - **no-mount**: `face_busy` is raised for the call's duration; a row
///   calling `tur::mount` inside traps (checked by the `mount` row).
/// - **depth-limited**: nested face calls (a derive reading a derived)
///   cap at [`VM_FACE_MAX_DEPTH`].
/// - **traps never abort the flush**: reported through the runtime-error
///   rail (worker→host `RuntimeError`), caller sees `Value::Nil`.
pub struct VmFace {
    vm: std::cell::RefCell<Weak<std::cell::RefCell<Vm>>>,
}

/// Nested face-call depth cap (a derive reading a derived reading a
/// derived…). Deep chains are a module bug; the call is reported + Nil.
pub const VM_FACE_MAX_DEPTH: u32 = 16;

/// Per-attempt fuel handed to a face call (the derive law's "fuel-capped").
pub const VM_FACE_FUEL: u64 = 200_000;
/// Bounded retries before the bail-out (a hostile derive can't wedge the
/// frame; an honest derive never comes close).
pub const VM_FACE_MAX_FUEL_RETRIES: u32 = 3;
/// Final drain budget: one last grant that drives a pathological call to
/// completion so the machine returns to IDLE (never left parked mid-flush).
pub const VM_FACE_DRAIN_FUEL: u64 = 2_000_000;

impl VmFace {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self {
            vm: std::cell::RefCell::new(Weak::new()),
        })
    }

    /// Install the VM (boot path) — the face holds a `Weak`, so teardown
    /// detaches every minted factory automatically.
    pub(crate) fn install(&self, vm: &Rc<std::cell::RefCell<Vm>>) {
        *self.vm.borrow_mut() = Rc::downgrade(vm);
    }

    /// Call an `entry fn` through the guards. `name` must be an export;
    /// `args` the (single) crossing argument. Errors are REPORTED (error
    /// rail) and returned — callers fall back to `Value::Nil`.
    pub fn call<A: CallArgs, R: Ret>(
        &self,
        handles: &RutHandles,
        name: &str,
        args: A,
    ) -> Result<R, rut_vm::Trap> {
        let vm = self
            .vm
            .borrow()
            .clone()
            .upgrade()
            .ok_or_else(|| {
                rut_vm::Trap::new(
                    rut_vm::TrapKind::Invalid,
                    format!("face call `{name}`: the rut module is gone"),
                )
            })?;
        let depth = handles.face_busy.get();
        if depth >= VM_FACE_MAX_DEPTH {
            report_runtime_error(
                handles,
                &format!("face call `{name}`: nested face depth {depth} exceeds the cap"),
            );
            return Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                "face depth cap exceeded",
            ));
        }
        handles.face_busy.set(depth + 1);
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = vm.borrow_mut();
            guard.add_fuel(VM_FACE_FUEL);
            match guard.call::<_, R>(name, args) {
                Ok(v) => Ok(v),
                Err(t) if t.kind == rut_vm::TrapKind::OutOfFuel => {
                    // Bounded retries, then one drain grant that always
                    // returns the machine to idle (never parked mid-flush).
                    let done;
                    let mut retries = 0;
                    loop {
                        let grant = if retries < VM_FACE_MAX_FUEL_RETRIES {
                            VM_FACE_FUEL
                        } else {
                            VM_FACE_DRAIN_FUEL
                        };
                        guard.add_fuel(grant);
                        match guard.resume::<R>() {
                            Ok(v) => {
                                done = Ok(v);
                                break;
                            }
                            Err(t2) if t2.kind == rut_vm::TrapKind::OutOfFuel
                                && retries < VM_FACE_MAX_FUEL_RETRIES =>
                            {
                                retries += 1;
                            }
                            Err(t2) => {
                                done = Err(t2);
                                break;
                            }
                        }
                    }
                    if let Err(t) = &done {
                        report_runtime_error(
                            handles,
                            &format!("face call `{name}`: out of fuel after bounded grants (total used {})", guard.fuel_used),
                        );
                        let _ = t;
                    }
                    done
                }
                Err(t) => {
                    report_runtime_error(handles, &format!("face call `{name}`: {} — {}", t.name(), t.msg));
                    Err(t)
                }
            }
        }));
        handles.face_busy.set(depth);
        match out {
            Ok(r) => r,
            Err(_) => Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Panic,
                format!("face call `{name}` panicked"),
            )),
        }
    }
}

/// Report a face trap through the runtime-error rail (worker → host), the
/// same channel JS runtime errors ride. Never aborts the caller.
pub(crate) fn report_runtime_error(handles: &RutHandles, message: &str) {
    tracing::error!("rut runtime error: {message}");
    let _ = handles.host_tx.unbounded_send(HostMsg::RuntimeError {
        report: crate::core::app::runtime_error::RuntimeErrorReport {
            message: message.to_string(),
            stack: None,
        },
    });
}

// ---------------------------------------------------------------------------
// RutRuntime — one per instance, lives beside the JS realm on the worker.
// ---------------------------------------------------------------------------

pub struct RutRuntime {
    /// The VM, in a shared cell (see `boot`): the rut rail's own calls take
    /// a transient borrow; view factories / deriveds reach it through the
    /// face's `Weak`.
    pub vm: Rc<std::cell::RefCell<Vm>>,
    handles: Rc<RutHandles>,
    /// The instance context (held for `apply_root` and future rails).
    js_ctx: TurInstanceContext,
    pub has_stop: bool,
    /// `entry fn start()`'s answer when declared `-> u64` (the module's
    /// handle back to the host — e.g. the id of its root atom), else 0.
    pub start_answer: u64,
}

impl RutRuntime {
    /// Convert an opaque handle minted by a rut row (`RutView`) back into
    /// the materialized view — the item-builder face's crossing back from
    /// an `entry fn(index) -> opaque`.
    pub fn view_of(handle: &OpaqueRef) -> Option<Rc<dyn View>> {
        let v = Opaque::<RutView>::from_handle(handle).ok()?;
        v.with(|v| Some(v.0.clone())).ok()?
    }
    /// Assemble a fresh session (core + the in-memory `tur` decl pkg) and
    /// compile `source` against it. Split from [`Self::boot`] so a
    /// syntactically-broken module fails BEFORE any teardown runs (the
    /// parse-first contract).
    fn compile(
        source: &str,
        exts: &[RutPkgExt],
    ) -> Result<(Rc<rut_core::binary::Program>, rut_vm::interp::HostPkgContext), String> {
        // The decl surface: the engine rows plus every extension's rows
        // (plugin-owned — tur-animation's C5 rows), so the compile sees the
        // full surface the boot will bind.
        let mut ext_decl: Vec<(String, Vec<TypeId>, TypeId, bool)> = Vec::new();
        let mut ext_consts: Vec<(String, TypeId, u64)> = Vec::new();
        let mut probe = rut_vm::interp::HostPkg::new("tur");
        for ext in exts {
            ext(&mut RutPkgCx {
                decl: &mut ext_decl,
                consts: &mut ext_consts,
                pkg: &mut probe,
                handles: None,
            });
        }
        let module = {
            let mut m = tur_decl_module();
            if let ModuleBody::Host { host_funcs, consts, .. } = &mut m.body {
                host_funcs.extend(ext_decl);
                consts.extend(ext_consts);
            }
            m
        };
        let mut session = rut_driver::Session::new();
        rut_driver::mount_std_core(&mut session);
        // The async weave (Future trait + the launch rows) — the C6 async
        // capability rows `await` through it.
        rut_driver::mount_std_async(&mut session);
        session
            .register_module("tur", module)
            .map_err(|e| format!("mount tur pkg: {e}"))?;

        let out = rut_driver::compile_module_in(&mut session, source, rut_parser::Mode::Impl, "app");
        if !out.diags.is_empty() {
            let msgs: Vec<String> = out.diags.iter().map(|d| d.msg.clone()).collect();
            return Err(msgs.join("; "));
        }
        let binary = out.binary.ok_or("rut compile emitted no binary")?;
        let prog = rut_core::binary::decode(&binary).map_err(|e| format!("decode: {e}"))?;
        rut_vm::verify::verify(&prog).map_err(|e| format!("verify: {e}"))?;
        Ok((Rc::new(prog), session.host_pkg_context()))
    }

    /// Parse + compile only (the parse-first half of the load contract) —
    /// a broken reload must fail before any teardown runs.
    pub fn parse_check(source: &str, exts: &[RutPkgExt]) -> Result<(), crate::core::app::ModuleError> {
        Self::compile(source, exts)
            .map(|_| ())
            .map_err(crate::core::app::ModuleError::Parse)
    }

    /// Bind bodies, verify the join, boot the VM, and invoke `start`.
    ///
    /// `realm_slot` + `arm` wire the realm face: the slot is the engine's
    /// shared realm storage, `arm` installs the realm constructor closure
    /// (the worker backend's `arm_realm_face`). A module whose rows never
    /// demand the realm never triggers construction.
    pub fn boot(
        source: &str,
        js_ctx: TurInstanceContext,
        realm: RutRealmInputs,
        exts: Vec<RutPkgExt>,
    ) -> Result<Self, crate::core::app::ModuleError> {
        let (prog, ctx) =
            Self::compile(source, &exts).map_err(crate::core::app::ModuleError::Parse)?;

        let face = VmFace::new();
        let handles: Rc<RutHandles> = Rc::new(RutHandles {
            store: js_ctx.store.clone(),
            dirty: js_ctx.dirty.clone(),
            element_tree: js_ctx.element_tree.clone(),
            focus_manager: js_ctx.focus_manager.clone(),
            pending_root: std::cell::RefCell::new(None),
            pending_calls: std::cell::RefCell::new(Vec::new()),
            click_seq: std::cell::Cell::new(0),
            stash: std::cell::RefCell::new(std::collections::HashMap::new()),
            host_tx: js_ctx.host_tx.clone(),
            realm: realm.face,
            clock: realm.clock,
            mutation_queue: js_ctx.mutation_queue.clone(),
            js_ctx: js_ctx.clone(),
            face,
            face_busy: std::cell::Cell::new(0),
        });

        let mut hosts = rut_vm::interp::HostRegistry::new();
        // The async launcher set (`__launch` / `__abort` / `__sleep`) — the
        // standard `mount_std_async` decls demand these bodies (the spike's
        // wiring).
        hosts.install_host_pkg(&ctx, rut_std::async_host::pkg());
        install_tur_pkg(&mut hosts, &ctx, &handles, &exts);
        hosts.verify_against(&ctx.flatten());

        let export_of = |name: &str| {
            prog.exports
                .iter()
                .find(|(n, _)| prog.interner.name(*n) == name)
                .map(|(_, fid)| *fid as usize)
        };
        let has_stop = export_of("stop").is_some();
        // `entry fn start() -> u64` hands the host a module answer (e.g.
        // its root atom's id); a plain `start()` returns nil.
        let start_returns_u64 = export_of("start")
            .and_then(|fid| prog.funcs.get(fid))
            .is_some_and(|f| f.ret == TY_U64);

        let vm = rut_vm::interp::Vm::new(
            prog,
            &default_limits(),
            rut_vm::interp::HostHooks::default(),
            hosts,
        )
        .map_err(|t| crate::core::app::ModuleError::Eval(format!("boot: {} — {}", t.name(), t.msg)))?;
        // The VM lives in a shared cell: view factories / deriveds minted
        // by rows reach it through the face's Weak (flush-time calls), so
        // they never hold a borrow across the engine's own `&mut Vm` calls.
        let vm = Rc::new(std::cell::RefCell::new(vm));
        handles.face.install(&vm);

        let mut rt = RutRuntime {
            vm,
            handles,
            js_ctx: js_ctx.clone(),
            has_stop,
            start_answer: 0,
        };
        rt.call_start(start_returns_u64)?;
        Ok(rt)
    }

    fn call_start(&mut self, returns_u64: bool) -> Result<(), crate::core::app::ModuleError> {
        let mut vm = self.vm.borrow_mut();
        if returns_u64 {
            let answer = vm
                .call::<_, u64>("start", ())
                .map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!("start: {} — {}", t.name(), t.msg))
                })?;
            self.start_answer = answer;
        } else {
            vm.call::<_, ()>("start", ())
                .map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!("start: {} — {}", t.name(), t.msg))
                })?;
        }
        Ok(())
    }

    /// Call a named `entry fn(u64, f64)` — the engine→rut event rail
    /// (input dispatch, embedder events). Runs OUTSIDE flush; rows must
    /// not mount (stash-and-apply is a `start`-time contract in Phase 2).
    pub fn call_entry(&mut self, name: &str, a: u64, b: f64) -> Result<(), crate::core::app::ModuleError> {
        self.vm
            .borrow_mut()
            .call::<_, ()>(name, (a, b))
            .map(|_| ())
            .map_err(|t| crate::core::app::ModuleError::Eval(format!("{name}: {} — {}", t.name(), t.msg)))
    }

    /// Apply the root stashed by `tur::mount` into the instance tree —
    /// the engine-side twin of the JS `mount(view)` bridge. **Realm-free**:
    /// rut rows materialize pure-Rust `Rc<dyn View>` values and the tree
    /// build path is realm-optional, so a rut-only instance never touches
    /// the JS realm here.
    pub fn apply_root(&mut self) -> Result<(), String> {
        let Some(user_view) = self.handles.pending_root.borrow_mut().take() else {
            return Ok(());
        };
        let tree = self.handles.element_tree.clone();

        // One-root invariant: replace any existing root (same as JS mount).
        if let Some(old) = tree.borrow().root_element_id() {
            tree.borrow_mut().destroy_subtree(old);
        }

        let root_view = RootView { child: user_view };
        let mut cx = SharedViewCx::new(self.js_ctx.clone(), None);
        let temp_parent = cx.alloc_node();
        let root_id = root_view.build(&mut cx, temp_parent);
        tree.borrow_mut()
            .set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));
        Ok(())
    }

    /// Drive ready rut tasks once (pump-level — never inside a flush).
    /// A launched task's trap rides the runtime-error rail (the same
    /// channel the face calls report through) — never a silent stall.
    pub fn run_ready(&mut self) {
        if let Err(t) = self.vm.borrow_mut().run_ready() {
            eprintln!("[rut-dbg] task trap: {} — {}", t.name(), t.msg);
            let msg = format!("rut task trap: {} — {}", t.name(), t.msg);
            let _ = self
                .handles
                .host_tx
                .unbounded_send(HostMsg::RuntimeError {
                    report: crate::core::app::runtime_error::RuntimeErrorReport {
                        message: msg,
                        stack: None,
                    },
                });
        }
    }

    /// Drain the callback intents queued by element callbacks this frame.
    /// Runs with the boa borrow RELEASED — rows may borrow the realm on
    /// demand (`rs_get_*`). Re-mount stashing is applied by the caller
    /// (the pump, which re-borrows for `apply_root` + the convergence
    /// flush). Returns the number of callbacks drained.
    pub fn drain_pending_calls(&mut self) -> usize {
        let calls: Vec<Intent> = std::mem::take(&mut *self.handles.pending_calls.borrow_mut());
        for intent in &calls {
            let name = intent_name(intent);
            let outcome = self.call_intent(intent);
            match outcome {
                Ok(()) => eprintln!("[rut-dbg] callback {name} ok"),
                Err(t) => eprintln!("[rut-dbg] callback {name} TRAP: {} — {}", t.name(), t.msg),
            }
        }
        calls.len()
    }

    /// Dispatch one intent into its `entry fn` (per-shape signatures).
    fn call_intent(&mut self, intent: &Intent) -> Result<(), rut_vm::Trap> {
        let mut vm = self.vm.borrow_mut();
        match intent {
            Intent::Click { name, a, b, seq } => vm.call::<_, ()>(name, (*a, *b, *seq)),
            Intent::Key { name, id, key, code, modifiers, kind } => {
                vm.call::<_, ()>(name, (*id, key.as_str(), code.as_str(), *modifiers, *kind))
            }
            Intent::Pointer { name, id, lx, ly, gx, gy, button } => {
                vm.call::<_, ()>(name, (*id, *lx, *ly, *gx, *gy, *button))
            }
            Intent::Value { name, a, value } => {
                let n = match value {
                    Value::Num(n) => *n,
                    Value::Bool(b) => *b as u64 as f64,
                    _ => 0.0,
                };
                vm.call::<_, ()>(name, (*a, n))
            }
            Intent::Bytes { name, a, data } => vm.call::<_, ()>(name, (*a, data.clone())),
        }
    }

    /// Best-effort `entry fn stop()` (the cleanup contract).
    pub fn stop(&mut self) {
        if self.has_stop
            && let Err(t) = self.vm.borrow_mut().call::<_, ()>("stop", ())
        {
            tracing::error!("rut module stop: {} — {}", t.name(), t.msg);
        }
        // Root teardown is engine-owned (teardown_current_module clears it).
        self.handles.pending_root.borrow_mut().take();
    }
}

/// The intent's entry-fn name (drain logging).
fn intent_name(intent: &Intent) -> &str {
    match intent {
        Intent::Click { name, .. }
        | Intent::Key { name, .. }
        | Intent::Pointer { name, .. }
        | Intent::Value { name, .. }
        | Intent::Bytes { name, .. } => name,
    }
}

/// The boot wiring `WorkerBackend::load_rut_module_inner` hands to
/// [`RutRuntime::boot`] — an already-armed realm face plus the engine
/// clock (the animation rows' `now_ms` source; the same `Clock` the
/// animation subsystem ticks with).
pub struct RutRealmInputs {
    pub face: RutRealm,
    pub clock: std::rc::Rc<dyn boa_engine::context::time::Clock>,
}

/// The pkg-extension context an installer sees: the `tur` host pkg's decl
/// rows + consts (compile side) and the body pkg + bridge handles (boot
/// side).
pub struct RutPkgCx<'a> {
    /// The decl rows `(name, params, ret, is_async)` appended before
    /// compilation (async rows ride the driver's family expansion).
    pub decl: &'a mut Vec<(String, Vec<TypeId>, TypeId, bool)>,
    /// The decl consts appended before compilation.
    pub consts: &'a mut Vec<(String, TypeId, u64)>,
    /// The body pkg the installer registers its rows into.
    pub pkg: &'a mut rut_vm::interp::HostPkg,
    /// The per-instance bridge handles — `None` at compile time (the decl
    /// probe), `Some` at boot.
    pub handles: Option<&'a Rc<RutHandles>>,
}

/// A rut pkg extension: plugin-owned rows for the `tur` host pkg (e.g.
/// tur-animation's C5 rows). Plugins push one during `register`; both the
/// compile (decl) and boot (bodies) phases drain them.
pub type RutPkgExt = Rc<dyn Fn(&mut RutPkgCx<'_>)>;
