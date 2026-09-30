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

use boa_engine::Context;

use crate::core::app::root::RootView;
use crate::core::edgy::reactive::{AtomId, Readable, Source, ScalarRead};
use crate::core::edgy::value::Value;
use crate::core::js_runtime::TurInstanceContext;
use crate::core::layout::Axis;
use crate::core::view::{SharedViewCx, View, ViewFactory, Val};
use crate::builtin_plugins::control_flow::ConditionView;
use crate::builtin_plugins::gesture::PointerInteractView;
use crate::builtin_plugins::layout::{ContainerView, FlexView, FlexibleView, PositionedView, StackView};
use crate::builtin_plugins::text::TextView;
use crate::core::layout::FlexFit;
use crate::core::render::brush::{Brush, Color};
use rut_core::types::{TypeId, TY_BOOL, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};
use rut_driver::ModuleBody;
use rut_vm::Opaque;

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
enum ViewBuilder {
    Flex { axis: Axis, children: Vec<Rc<dyn View>> },
    Stack { children: Vec<Rc<dyn View>> },
}

impl ViewBuilder {
    fn materialize(self) -> Rc<dyn View> {
        match self {
            ViewBuilder::Flex { axis, children } => Rc::new(FlexView {
                direction: Some(axis),
                main_alignment: None,
                cross_alignment: None,
                main_axis_size: None,
                children,
                query_key: None,
            }),
            ViewBuilder::Stack { children } => Rc::new(StackView {
                fit: None,
                alignment: None,
                children,
                query_key: None,
            }),
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
    let host_funcs: Vec<(String, Vec<TypeId>, TypeId, bool)> = vec![
        row("el_column", vec![], TY_OPAQUE),
        row("el_row", vec![], TY_OPAQUE),
        row("el_text", vec![TY_STR], TY_OPAQUE),
        row("el_text_bound", vec![TY_U64], TY_OPAQUE),
        row("el_text_styled", vec![TY_STR, TY_F64, TY_U64], TY_OPAQUE),
        row("el_scroll", vec![TY_BOOL, TY_OPAQUE], TY_OPAQUE),
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
    ];
    rut_driver::Module {
        namespace: Some("tur".to_string()),
        body: ModuleBody::Host {
            host_funcs,
            consts: Vec::new(),
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
) {
    let mut pkg = rut_vm::interp::HostPkg::new("tur");

    // mint a flex builder
    rut_vm::pkg_fn!(pkg, "el_column", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Flex { axis: Axis::Vertical, children: Vec::new() })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_row", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Flex { axis: Axis::Horizontal, children: Vec::new() })?.handle().clone())
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
            let dummy = ViewBuilder::Flex { axis: Axis::Vertical, children: Vec::new() };
            std::mem::replace(b, dummy).materialize()
        })?;
        Ok(Opaque::alloc(vm, RutView(built))?.handle().clone())
    });
    // attach a materialized child to a flex builder
    rut_vm::pkg_fn!(pkg, "el_child", (Opaque<ViewBuilder>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, child: Opaque<RutView>| {
        let child_view = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, b| {
            match b {
                // The only multi-child builders today; the match keeps
                // future kinds honest.
                ViewBuilder::Flex { children, .. } | ViewBuilder::Stack { children } => {
                    children.push(child_view);
                }
            }
        })?;
        Ok(())
    });
    // stash the root — the engine applies it after `start` returns
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mount", (Opaque<RutView>,) -> (), move |vm: &mut rut_vm::interp::Vm, view: Opaque<RutView>| {
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
            h2.pending_calls.borrow_mut().push((cb.clone(), id_a, id_b, n as f64));
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
        Ok(Opaque::alloc(vm, ViewBuilder::Stack { children: Vec::new() })?.handle().clone())
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
    /// The root stashed by `tur::mount` during `start`, applied by the
    /// engine after the call returns (outside the VM, on the mount path).
    pub pending_root: std::cell::RefCell<Option<Rc<dyn View>>>,
    /// Callback intents queued by element callbacks (the rut closure
    /// closures are boa-free: they only push here). Drained at pump level
    /// after flush — `(callback name, id, payload)`.
    pub pending_calls: std::cell::RefCell<Vec<(String, u64, u64, f64)>>,
    /// Monotonic click counter stamped into click intents.
    pub click_seq: std::cell::Cell<u64>,
}

// ---------------------------------------------------------------------------
// RutRuntime — one per instance, lives beside the JS realm on the worker.
// ---------------------------------------------------------------------------

pub struct RutRuntime {
    pub vm: rut_vm::interp::Vm,
    handles: Rc<RutHandles>,
    /// The instance context (held for `apply_root` and future rails).
    js_ctx: TurInstanceContext,
    pub has_stop: bool,
    /// `entry fn start()`'s answer when declared `-> u64` (the module's
    /// handle back to the host — e.g. the id of its root atom), else 0.
    pub start_answer: u64,
}

impl RutRuntime {
    /// Assemble a fresh session (core + the in-memory `tur` decl pkg) and
    /// compile `source` against it. Split from [`Self::boot`] so a
    /// syntactically-broken module fails BEFORE any teardown runs (the
    /// parse-first contract).
    fn compile(
        source: &str,
    ) -> Result<(Rc<rut_core::binary::Program>, rut_vm::interp::HostPkgContext), String> {
        let mut session = rut_driver::Session::new();
        rut_driver::mount_std_core(&mut session);
        session
            .register_module("tur", tur_decl_module())
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
    pub fn parse_check(source: &str) -> Result<(), crate::core::app::ModuleError> {
        Self::compile(source)
            .map(|_| ())
            .map_err(crate::core::app::ModuleError::Parse)
    }

    /// Bind bodies, verify the join, boot the VM, and invoke `start`.
    pub fn boot(
        source: &str,
        js_ctx: TurInstanceContext,
    ) -> Result<Self, crate::core::app::ModuleError> {
        let (prog, ctx) = Self::compile(source).map_err(crate::core::app::ModuleError::Parse)?;

        let handles: Rc<RutHandles> = Rc::new(RutHandles {
            store: js_ctx.store.clone(),
            dirty: js_ctx.dirty.clone(),
            element_tree: js_ctx.element_tree.clone(),
            pending_root: std::cell::RefCell::new(None),
            pending_calls: std::cell::RefCell::new(Vec::new()),
            click_seq: std::cell::Cell::new(0),
        });

        let mut hosts = rut_vm::interp::HostRegistry::new();
        install_tur_pkg(&mut hosts, &ctx, &handles);
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
        if returns_u64 {
            let answer = self
                .vm
                .call::<_, u64>("start", ())
                .map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!("start: {} — {}", t.name(), t.msg))
                })?;
            self.start_answer = answer;
        } else {
            self.vm
                .call::<_, ()>("start", ())
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
    pub fn run_ready(&mut self) {
        if let Err(t) = self.vm.run_ready() {
            tracing::error!("rut run_ready trap: {} — {}", t.name(), t.msg);
        }
    }

    /// Drain the callback intents queued by element callbacks this frame.
    /// Runs with the boa borrow RELEASED — rows may borrow the realm on
    /// demand (`rs_get_*`). Re-mount stashing is applied by the caller
    /// (the pump, which re-borrows for `apply_root` + the convergence
    /// flush). Returns the number of callbacks drained.
    pub fn drain_pending_calls(&mut self) -> usize {
        let calls: Vec<(String, u64, u64, f64)> =
            std::mem::take(&mut *self.handles.pending_calls.borrow_mut());
        for (name, a, b, c) in &calls {
            if let Err(t) = self.vm.call::<_, ()>(name, (*a, *b, *c)) {
                eprintln!("[rut-dbg] callback {name}({a},{b},{c}) TRAP: {} — {}", t.name(), t.msg);
            } else {
                eprintln!("[rut-dbg] callback {name}({a},{b},{c}) ok");
            }
        }
        calls.len()
    }

    /// Best-effort `entry fn stop()` (the cleanup contract).
    pub fn stop(&mut self) {
        if self.has_stop
            && let Err(t) = self.vm.call::<_, ()>("stop", ())
        {
            tracing::error!("rut module stop: {} — {}", t.name(), t.msg);
        }
        // Root teardown is engine-owned (teardown_current_module clears it).
        self.handles.pending_root.borrow_mut().take();
    }
}
