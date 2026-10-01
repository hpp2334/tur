//! The rut runtime seam — the engine's script-rail mechanism.
//!
//! Architecture: **rut drives, the engine applies.** A loaded rut module's
//! `start()` builds a tree of *pure-Rust view data* through host rows and
//! stashes the root via `tur::mount`. The engine applies the stashed root
//! into the instance's `ElementTree` right after `start` returns, on the
//! same code path the JS `mount(view)` bridge used. The rut VM itself is
//! driven by the embedder's pump (`run_ready()` before each flush — never
//! inside a flush iteration).
//!
//! Module lifecycle contract (mirrors the JS contract): `entry fn start()`
//! is invoked after boot; `entry fn stop()` — if present — runs (best-effort)
//! before the next load and at destroy; the engine owns root-tree teardown.
//!
//! ## Layering law
//!
//! This module is MECHANISM ONLY: the `RutView` crossing, the
//! [`RutHandles`] bridge state, the `tur` host pkg's store / stash / mount
//! rows, the entry rails ([`Intent`] + the drain), and the pkg-extension
//! seam ([`RutPkgExt`]). It contains ZERO element concepts — every element
//! family's spec + rows live in the builtin plugin that owns its view type
//! (pushed via `PluginRegisterContext::push_rut_ext`, the
//! [`tur-animation`](https://docs.rs) installer pattern), and the authored
//! builder surface (the kit) lives outside `core/` entirely.

use std::rc::Rc;
use std::rc::Weak;

use crate::core::app::root::RootView;
use crate::core::app::HostMsg;
use crate::core::edgy::reactive::{AtomId, Readable, Source, ScalarRead};
use crate::core::edgy::value::Value;
use crate::core::instance::InstanceContext;
use crate::core::render::brush::Color;
use crate::core::view::{SharedViewCx, View};
use rut_core::types::{TypeId, TY_BOOL, TY_F64, TY_NIL, TY_OPAQUE, TY_OPT_OPAQUE, TY_STR, TY_U64};
use rut_driver::ModuleBody;
use rut_vm::Opaque;
use rut_vm::interp::{CallArgs, Ret, Vm};
use rut_vm::OpaqueRef;

mod async_caps;
mod derive;

/// The `RutView`-opaque → `Rc<dyn View>` crossing (item builders return
/// opaques from `entry fn(index)` calls).
pub fn opaque_to_view(handle: &OpaqueRef) -> Option<Rc<dyn View>> {
    RutRuntime::view_of(handle)
}

/// Rebuild a source handle from a raw atom id — the rut rows' crossing
/// (the ids ARE the atoms). Engine-pub so the pkg extensions (plugin-owned
/// rut rows) can bind reactive props.
pub fn source_of<T>(atom: u64) -> Source<T> {
    Source::from_id(AtomId(atom as u32))
}

/// [`source_of`] as a `Readable`.
pub fn readable_of<T>(atom: u64) -> Readable<T> {
    Readable::Source(source_of(atom))
}

/// `0xRRGGBBAA` packed color → engine `Color` (the packed-color crossing
/// every styled row family shares).
pub fn color_of(packed: u64) -> Color {
    Color::rgba(
        ((packed >> 24) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

/// A materialized view (`Rc<dyn View>`) sealed in an opaque box.
pub struct RutView(pub Rc<dyn View>);

/// A native edgy [`Value`] sealed in an opaque box — the rut rail's handle
/// to structured atom values (lists / maps).
pub struct RutValue(pub Value);

/// A mutable f64 cell — the stateful-entry scratch crossing (the stash
/// holds opaques only, so numbers cross in cells).
pub struct RutCell(pub std::cell::Cell<f64>);

/// The per-instance resource budget. Phase-1 defaults; tunable per embedder.
pub fn default_limits() -> rut_vm::interp::Limits {
    rut_vm::interp::Limits {
        fuel: Some(50_000_000),
        heap_limit_bytes: Some(64 * 1024 * 1024),
        interrupt_every: 1024,
    }
}

// ---------------------------------------------------------------------------
// The `tur` host package — decl rows (mounted in-memory as a Module) +
// bodies (a HostPkg installed into the per-instance HostRegistry).
//
// MECHANISM ONLY: mount + the C8 no-mount trap, the `rs_*` store rows,
// and the stash / scratch rails. Element rows live in the plugins that own
// their view types and arrive through the [`RutPkgExt`] seam.
// ---------------------------------------------------------------------------

/// The in-memory `tur` host-pkg Module (the DECL side): the mechanism
/// surface rut code compiles against. Mounted via `Session::register_module`
/// — no filesystem involved. Plugin families extend it through their
/// pushed [`RutPkgExt`]s (decl rows + consts).
pub fn tur_decl_module() -> rut_driver::Module {
    let row = |name: &str, params: Vec<TypeId>, ret: TypeId| (name.to_string(), params, ret, false);
    let host_funcs: Vec<(String, Vec<TypeId>, TypeId, bool)> = vec![
        // stash the root — the engine applies it after `start` returns
        row("mount", vec![TY_OPAQUE], TY_NIL),
        // reactive rails: str / f64 / bool scalars over the native KV
        row("rs_source_str", vec![TY_STR], TY_U64),
        row("rs_set_str", vec![TY_U64, TY_STR], TY_NIL),
        row("rs_get_str", vec![TY_U64], TY_STR),
        row("rs_source_f64", vec![], TY_U64),
        row("rs_set_f64", vec![TY_U64, TY_F64], TY_NIL),
        row("rs_get_f64", vec![TY_U64], TY_F64),
        row("rs_source_bool", vec![TY_BOOL], TY_U64),
        row("rs_set_bool", vec![TY_U64, TY_BOOL], TY_NIL),
        row("rs_get_bool", vec![TY_U64], TY_BOOL),
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
        // brush atoms (nonzero packed color sets, 0 clears)
        row("rs_set_brush", vec![TY_U64, TY_U64], TY_NIL),
        // C8 — derived atoms + watch (the guarded flush-time VM call)
        row("rs_derive", vec![TY_STR, TY_U64], TY_U64),
        row("rs_derive2", vec![TY_STR, TY_U64, TY_U64], TY_U64),
        row("rs_watch", vec![TY_U64, TY_STR, TY_U64], TY_OPAQUE),
        row("rs_watch_start", vec![TY_OPAQUE], TY_NIL),
        row("rs_watch_stop", vec![TY_OPAQUE], TY_NIL),
        // the opaque stash (cross-entry hand-off)
        row("st_put", vec![TY_U64, TY_OPAQUE], TY_NIL),
        row("st_take", vec![TY_U64], TY_OPT_OPAQUE),
        // the scalar stash + scratch cells (atom ids / counts cross entries
        // and async frames as f64 — the opaque stash cannot hold numbers)
        row("stf_put", vec![TY_U64, TY_F64], TY_NIL),
        row("stf_take", vec![TY_U64], TY_F64),
        row("mem_new", vec![TY_F64], TY_OPAQUE),
        row("mem_get", vec![TY_OPAQUE], TY_F64),
        row("mem_set", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("str_parse_f64", vec![TY_STR], TY_F64),
    ];
    // C6 — async capabilities (clipboard + bytes helpers; the async rows
    // ride the driver's five-row family expansion).
    let mut funcs = host_funcs;
    funcs.extend(async_caps::decl_rows());
    rut_driver::Module {
        namespace: Some("tur".to_string()),
        body: ModuleBody::Host {
            host_funcs: funcs,
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
/// over (the pending root stash; the dispatch registry + reactive rails).
fn install_tur_pkg(
    hosts: &mut rut_vm::interp::HostRegistry,
    ctx: &rut_vm::interp::HostPkgContext,
    handles: &Rc<RutHandles>,
    exts: &[RutPkgExt],
) {
    let mut pkg = rut_vm::interp::HostPkg::new("tur");

    // ---- mount + the C8 no-mount law ------------------------------------
    //
    // A face-driven call (a derive / item builder materializing mid-flush)
    // may NOT re-mount — the trap is reported through the error rail and
    // the flush continues.
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

    // ---- reactive rails (the native-KV substrate) ------------------------
    //
    // Atoms are the engine's edgy store (`core::edgy`) — the SAME KV the
    // element tree uses — addressed by raw `AtomId` as u64. The KV holds
    // native `Value`s, so every row below is realm-free. Writes cross;
    // reads are served by the rut-side mirror (the wrapper atoms hold
    // their current value). The flush fixed-point (stale atoms → dirty
    // subscribers → re-layout) is entirely the engine's existing
    // machinery: a bound Text re-renders on `rs_set_*` with zero new
    // engine code.
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

    // ---- structured values ------------------------------------------------
    //
    // List / map atoms over the native `Value` KV, addressed through
    // opaque value handles (`RutValue`). Build in-place on an opaque
    // handle, then bind whole values to atoms:
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

    // Brush atoms: nonzero packed color sets it, 0 clears (Nil — the decode
    // fails and the prop resolves to absent). The set color wraps the
    // engine's Color opaque (the `FromValue for Brush` decode).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_brush", (u64, u64) -> (), move |_vm: &mut rut_vm::interp::Vm, atom: u64, color: u64| {
        let value = if color == 0 {
            crate::core::edgy::Value::Nil
        } else {
            let c = color_of(color);
            crate::core::edgy::Value::opaque(Rc::new(c) as Rc<dyn std::any::Any>)
        };
        h.store
            .bridge()
            .set_source(crate::core::edgy::reactive::Source::<crate::core::edgy::Value>::from_id(crate::core::edgy::reactive::AtomId(atom as u32)), value)
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_brush: {e}")))
    });

    // ---- the opaque + scalar stashes (cross-entry hand-off rails) --------
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
    {
        let h = handles.clone();
        rut_vm::pkg_fn!(pkg, "stf_put", (u64, f64) -> (), move |_vm: &mut rut_vm::interp::Vm, key: u64, v: f64| {
            h.stash_num.borrow_mut().insert(key, v);
            Ok(())
        });
        let h = handles.clone();
        rut_vm::pkg_fn!(pkg, "stf_take", (u64,) -> f64, move |_vm: &mut rut_vm::interp::Vm, key: u64| {
            Ok(h.stash_num.borrow_mut().remove(&key).unwrap_or(0.0))
        });
    }

    // ---- scratch cells + parse helper --------------------------------------
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

    // C6 — async capabilities (clipboard + the bytes helpers).
    async_caps::install(&mut pkg, handles);
    // C8 — derived atoms + watch (the guarded flush-time VM call).
    derive::install(&mut pkg, handles);

    // Plugin extensions (the element families + capability crates) — AFTER
    // the engine rows, so an extension may lean on them.
    let mut ext_decl = Vec::new();
    let mut ext_consts = Vec::new();
    let mut ext_preludes = Vec::new();
    for ext in exts {
        ext(&mut RutPkgCx {
            decl: &mut ext_decl,
            consts: &mut ext_consts,
            pkg: &mut pkg,
            handles: Some(handles),
            preludes: &mut ext_preludes,
        });
    }

    hosts.install_host_pkg(ctx, pkg);
}
// ---------------------------------------------------------------------------
// RutHandles — per-instance bridge state shared with the row closures.
// ---------------------------------------------------------------------------

pub struct RutHandles {
    /// The instance's reactive store — the SAME KV the element tree
    /// shares; rut atoms are edgy atoms addressed by raw id.
    pub store: crate::core::edgy::reactive::Store,
    /// The instance's app-dirty flag (rut callbacks raise it when they
    /// stash work so an idle worker wakes).
    pub dirty: Rc<std::cell::Cell<bool>>,
    /// The instance-owned tree handle — `apply_root` builds into it.
    pub element_tree: crate::core::elements::NodeTree,
    /// The instance's focus manager — the `focus_request` row targets it
    /// (the FocusChange flush pushes the focus/blur mutations next frame).
    pub focus_manager: Rc<std::cell::RefCell<crate::core::focus::FocusManager>>,
    /// The root stashed by `tur::mount` during `start`, applied by the
    /// engine after the call returns (outside the VM, on the mount path).
    pub pending_root: std::cell::RefCell<Option<Rc<dyn View>>>,
    /// Callback intents queued by element callbacks (the row closures are
    /// realm-free: they only push here). Drained at pump level after
    /// flush — `(callback name, id, payload)`.
    pub pending_calls: std::cell::RefCell<Vec<Intent>>,
    /// Monotonic click counter stamped into click intents.
    pub click_seq: std::cell::Cell<u64>,
    /// The module-facing opaque stash — `st_put` / `st_take` let a module
    /// hold host objects across entry calls (an async frame cannot carry
    /// opaque params, so the stash is the hand-off rail).
    pub stash: std::cell::RefCell<std::collections::HashMap<u64, OpaqueRef>>,
    /// The scalar stash (`stf_put` / `stf_take`) — atom ids and counts
    /// cross entries and async frames as f64.
    pub stash_num: std::cell::RefCell<std::collections::HashMap<u64, f64>>,
    /// The worker→host channel — runtime-error reports for face traps ride
    /// the same `RuntimeError` message the JS rail used.
    pub host_tx: crate::core::app::HostTx,
    /// The engine's shared clock — the animation rows' `now_ms` source.
    pub clock: Rc<dyn crate::core::clock::Clock>,
    /// The engine-wide mutation queue — animation `onTick` callbacks ride
    /// it (same dispatch path the JS controllers used).
    pub mutation_queue: Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>,
    /// The instance context — capability lookups + worker-side spawns (the
    /// async capability rows: clipboard / net / filepicker).
    pub inst: InstanceContext,
    /// The flush-time VM face: view factories / deriveds minted
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
/// record payloads (keys, pointer positions, raw values) that the
/// gesture / animation / watch rails queue.
#[derive(Clone, Debug)]
pub enum Intent {
    /// The tap shape: `(name, id_a, id_b, seq)`.
    Click { name: String, a: u64, b: u64, seq: f64 },
    /// A key event from the Focusable's `onKeyDown` mutation.
    Key { name: String, id: u64, key: String, code: String, modifiers: u64, kind: u64 },
    /// A pointer event from the PointerInteract down/move/up/context-menu
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
    /// The two-id pointer variant (the two-id gesture rail):
    /// `(name, id_a, id_b, positions…)`.
    Pointer2 {
        name: String,
        a: u64,
        b: u64,
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
/// same channel JS runtime errors rode. Never aborts the caller.
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
// RutRuntime — one per instance, lives on the worker.
// ---------------------------------------------------------------------------

pub struct RutRuntime {
    /// The VM, in a shared cell (see `boot`): the rut rail's own calls take
    /// a transient borrow; view factories / deriveds reach it through the
    /// face's `Weak`.
    pub vm: Rc<std::cell::RefCell<Vm>>,
    handles: Rc<RutHandles>,
    /// The instance context (held for `apply_root` and future rails).
    js_ctx: InstanceContext,
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
    /// Assemble a fresh session (core + the in-memory `tur` decl pkg +
    /// every extension's prelude modules) and compile `source` against it.
    /// Split from [`Self::boot`] so a syntactically-broken module fails
    /// BEFORE any teardown runs (the parse-first contract).
    fn compile(
        source: &str,
        exts: &[RutPkgExt],
    ) -> Result<(Rc<rut_core::binary::Program>, rut_vm::interp::HostPkgContext), String> {
        // The decl surface: the engine rows plus every extension's rows
        // (plugin-owned — the element families + capability crates), so
        // the compile sees the full surface the boot will bind. Extensions
        // also contribute prelude modules (the kit — the authored builder
        // surface, owned by the standard bundle assembly, never by core).
        let mut ext_decl: Vec<(String, Vec<TypeId>, TypeId, bool)> = Vec::new();
        let mut ext_consts: Vec<(String, TypeId, u64)> = Vec::new();
        let mut preludes: Vec<(String, rut_driver::Module)> = Vec::new();
        let mut probe = rut_vm::interp::HostPkg::new("tur");
        for ext in exts {
            ext(&mut RutPkgCx {
                decl: &mut ext_decl,
                consts: &mut ext_consts,
                pkg: &mut probe,
                handles: None,
                preludes: &mut preludes,
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
        // capability rows `await` through it. On wasm the weave's mount
        // reads the toolchain tree from disk (rut-driver's
        // `mount_std_async` canonicalizes a checkout path) — unavailable,
        // so the weave is native-only for now: non-async rut modules load
        // on the web, async ones fail the compile with unknown-module
        // diagnostics.
        if !cfg!(target_arch = "wasm32") {
            rut_driver::mount_std_async(&mut session);
        }
        session
            .register_module("tur", module)
            .map_err(|e| format!("mount tur pkg: {e}"))?;
        // The preludes (the kit et al.) — registered after `tur`, whose
        // rows they wrap.
        for (spec, module) in preludes {
            session
                .register_module(&spec, module)
                .map_err(|e| format!("mount prelude `{spec}`: {e}"))?;
        }

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
    pub fn boot(
        source: &str,
        js_ctx: InstanceContext,
        inputs: RutRealmInputs,
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
            stash_num: std::cell::RefCell::new(std::collections::HashMap::new()),
            host_tx: js_ctx.host_tx.clone(),
            clock: inputs.clock,
            mutation_queue: js_ctx.mutation_queue.clone(),
            inst: js_ctx.clone(),
            face,
            face_busy: std::cell::Cell::new(0),
        });

        let mut hosts = rut_vm::interp::HostRegistry::new();
        // The async launcher set (`__launch` / `__abort` / `__sleep`) — the
        // standard `mount_std_async` decls demand these bodies (the spike's
        // wiring). Native-only, symmetric with the decl mount above.
        if !cfg!(target_arch = "wasm32") {
            hosts.install_host_pkg(&ctx, rut_std::async_host::pkg());
        }
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
    /// a script realm here.
    pub fn apply_root(&mut self) -> Result<(), String> {
        let Some(user_view) = self.handles.pending_root.borrow_mut().take() else {
            return Ok(());
        };
        let tree = self.handles.element_tree.clone();

        // One-root invariant: replace any existing root (same as JS mount).
        // The read drops its borrow BEFORE the destroy borrows mutably —
        // an `if let` scrutinee temporary would hold the shared borrow
        // across the body and panic the `borrow_mut` on every reload.
        let old = tree.borrow().root_element_id();
        if let Some(old) = old {
            tree.borrow_mut().destroy_subtree(old);
        }

        let root_view = RootView { child: user_view };
        let mut cx = SharedViewCx::new(self.js_ctx.clone());
        let temp_parent = cx.alloc_node();
        let root_id = root_view.build(&mut cx, temp_parent);
        tree.borrow_mut()
            .set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));
        Ok(())
    }

    /// Drive ready rut tasks once (pump-level — never inside a flush).
    /// The VM's virtual clock syncs to the engine clock first, and due
    /// sleep timers expire into the ready queue (`sleep` deadlines arm
    /// against `set_now`). A launched task's trap rides the
    /// runtime-error rail (the same channel the face calls report
    /// through) — never a silent stall.
    pub fn run_ready(&mut self) {
        {
            let mut vm = self.vm.borrow_mut();
            let now = self.handles.clock.now_millis();
            vm.set_now(now as u64);
            let _ = vm.next_deadline();
        }
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
    /// Re-mount stashing is applied by the caller (the pump, which runs
    /// `apply_root` + the convergence flush). Returns the number of
    /// callbacks drained.
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
            Intent::Pointer2 { name, a, b, lx, ly, gx, gy, button } => {
                vm.call::<_, ()>(name, (*a, *b, *lx, *ly, *gx, *gy, *button))
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
        | Intent::Pointer2 { name, .. }
        | Intent::Value { name, .. }
        | Intent::Bytes { name, .. } => name,
    }
}

/// The boot wiring `WorkerBackend::load_rut_module_inner` hands to
/// [`RutRuntime::boot`] — the engine clock (the rows' `now_ms` source; the
/// same `Clock` the animation subsystem ticks with).
pub struct RutRealmInputs {
    pub clock: std::rc::Rc<dyn crate::core::clock::Clock>,
}

/// The pkg-extension context an installer sees: the `tur` host pkg's decl
/// rows + consts (compile side), the body pkg + bridge handles (boot side),
/// and the prelude modules (the kit et al.) registered before the app
/// source compiles.
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
    /// Prelude rut modules `(spec, module)` registered into the compile
    /// session before the app source compiles (the kit — the authored
    /// builder surface). Core never fills this; the standard bundle
    /// assembly does.
    pub preludes: &'a mut Vec<(String, rut_driver::Module)>,
}

/// A rut pkg extension: plugin-owned rows for the `tur` host pkg (e.g.
/// tur-animation's C5 rows, each element family's spec + rows). Plugins
/// push one during `register`; both the compile (decl) and boot (bodies)
/// phases drain them.
pub type RutPkgExt = Rc<dyn Fn(&mut RutPkgCx<'_>)>;
