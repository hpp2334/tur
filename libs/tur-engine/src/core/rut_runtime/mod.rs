//! The rut runtime seam — the engine's script-rail mechanism.
//!
//! Architecture: **rut drives, the engine applies.** A loaded rut module's
//! `start()` builds a tree of *pure-Rust view data* through host rows and
//! stashes the root via `tur_host::mount`. The engine applies the stashed root
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
//! [`RutHandles`] bridge state, the `tur_host` pkg's store / mount rows,
//! the entry rails ([`Intent`] + the drain), and the pkg-extension
//! seam ([`RutPkgExt`]). It contains ZERO element concepts — every element
//! family's spec + rows live in the builtin plugin that owns its view type
//! (pushed via `PluginRegisterContext::push_rut_ext`, the
//! [`tur-animation`](https://docs.rs) installer pattern), and the authored
//! builder surface (the kit) lives outside `core/` entirely.

use std::rc::Rc;
use std::rc::Weak;

use crate::core::app::HostMsg;
use crate::core::app::root::RootView;
use crate::core::edgy::reactive::{AtomId, Readable, ScalarRead, Source};
use crate::core::edgy::value::Value;
use crate::core::instance::InstanceContext;
use crate::core::render::brush::Color;
use crate::core::view::{SharedViewCx, View};
use rut_core::types::{TY_BOOL, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64, TypeId};
use rut_driver::PkgBody;
use rut_vm::Opaque;
use rut_vm::OpaqueRef;
use rut_vm::interp::{CallArgs, Ret, Vm};

mod async_caps;
mod decl_gen;
mod derive;
mod mutation;

pub use decl_gen::{render_tur_host_decl, tur_host_surface};
pub use mutation::{CtxBridge, ValueArgs, mutation_of};

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
    Color::from_packed(packed)
}

/// The boxed rail's number stamps (`rs_box_stamp` / `ctx_get_box`): the
/// KV holds ONE number variant, so the mint reports the box's integer- vs
/// float-ness and the read re-stamps the recovered box to match.
pub const STAMP_NUM_F64: u64 = 1;
pub const STAMP_NUM_U64: u64 = 2;

/// Open a rut erasure box (or unwrap a host box) into the native KV
/// value — the boxed rail's decode, shared by `rs_source_box` and
/// `ctx_set_box`. The payload kind picks the KV variant; a box holding
/// an opaque unwraps the structured lane's `RutValue` host box (lists /
/// maps land in the KV as native values, the shape the bound-prop rows
/// read) and rides the raw host-box lane otherwise (the controller-slot
/// shape, readable through `ctx_get_opaque`).
pub(crate) fn boxed_to_value(vm: &Vm, h: &OpaqueRef) -> Result<Value, rut_vm::Trap> {
    match rut_vm::rut_box_payload(vm, h) {
        Ok((_, v)) => match v {
            rut_vm::Value::Nil => Ok(Value::Nil),
            rut_vm::Value::I64(n) => Ok(Value::Num(n as u64 as f64)),
            rut_vm::Value::F64(f) => Ok(Value::Num(f)),
            rut_vm::Value::Bool(b) => Ok(Value::Bool(b)),
            rut_vm::Value::Str(s) => Ok(Value::str(s)),
            rut_vm::Value::Bytes(b) => Ok(Value::Bytes(Rc::from(b.into_boxed_slice()))),
            rut_vm::Value::Opaque(inner) => match Opaque::<RutValue>::from_handle(&inner) {
                Ok(rv) => Ok(rv.with(|v| v.0.clone())?),
                Err(_) => Ok(Value::opaque(Rc::new(inner) as Rc<dyn std::any::Any>)),
            },
            other => Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                format!(
                    "the boxed lane carries scalars / strings / bytes / opaques — got {}",
                    other.kind_name()
                ),
            )),
        },
        // A raw host payload box: the structured-value lane first, else
        // the raw host-box lane.
        Err(_) => match Opaque::<RutValue>::from_handle(h) {
            Ok(rv) => Ok(rv.with(|v| v.0.clone())?),
            Err(_) => Ok(Value::opaque(Rc::new(h.clone()) as Rc<dyn std::any::Any>)),
        },
    }
}

/// Seal a rut value the kit handed back as an opaque into a rut erasure
/// box stamped `opaque` — the box-in-box: the entry owns one reference to
/// the inner slot and `unopaque<opaque>` recovers it verbatim (the
/// mint→crossing ownership law, `opaque_handle_take`'s shape).
fn seal_opaque_box(vm: &mut Vm, owned: OpaqueRef) -> Result<OpaqueRef, rut_vm::Trap> {
    let slot = rut_vm::Slot { r: owned.ptr() };
    std::mem::forget(owned);
    vm.seal_opaque(slot, TY_OPAQUE)
}

/// Seal a native KV value back into a rut erasure box the calling rut
/// code can `unopaque<T>` — the boxed rail's encode, `ctx_get_box`'s
/// body. `stamp` picks the number lane's width (`STAMP_NUM_F64` /
/// `STAMP_NUM_U64`); every other kind stamps by its KV variant.
pub(crate) fn seal_box(vm: &mut Vm, value: &Value, stamp: u64) -> Result<OpaqueRef, rut_vm::Trap> {
    use rut_core::types::{TY_BOOL, TY_BYTES, TY_NIL, TY_STR};
    match value {
        Value::Nil => vm.seal_opaque(rut_vm::Slot::null(), TY_NIL),
        Value::Bool(b) => vm.seal_opaque(rut_vm::Slot::bool(*b), TY_BOOL),
        Value::Num(n) => {
            if stamp == STAMP_NUM_U64 {
                vm.seal_opaque(rut_vm::Slot::int(*n as i64), TY_U64)
            } else {
                vm.seal_opaque(rut_vm::Slot::float(*n), TY_F64)
            }
        }
        Value::Str(s) => {
            let slot = vm.alloc_str_cell(s.to_string())?;
            vm.seal_opaque(slot, TY_STR)
        }
        Value::Bytes(b) => {
            let slot = vm.alloc_bytes_cell(b.to_vec())?;
            vm.seal_opaque(slot, TY_BYTES)
        }
        // Opaques and structured values cross box-in-box: the recovered
        // opaque is the inner handle (controllers) or the minted
        // `RutValue` host box (lists / maps — the value ops' currency).
        Value::Opaque(any) => {
            let inner = any
                .downcast_ref::<OpaqueRef>()
                .ok_or_else(|| {
                    rut_vm::Trap::new(
                        rut_vm::TrapKind::Invalid,
                        "ctx_get_box: the atom holds a host value that does not cross the boxed lane",
                    )
                })?
                .clone();
            seal_opaque_box(vm, inner)
        }
        Value::List(_) | Value::Map(_) => {
            let host: Opaque<RutValue> = Opaque::alloc(vm, RutValue(value.clone()))?;
            let handle = host.handle().clone();
            std::mem::forget(host);
            let slot = rut_vm::Slot { r: handle.ptr() };
            vm.seal_opaque(slot, TY_OPAQUE)
        }
    }
}

/// A materialized view (`Rc<dyn View>`) sealed in an opaque box.
pub struct RutView(pub Rc<dyn View>);

/// A native edgy [`Value`] sealed in an opaque box — the rut rail's handle
/// to structured atom values (lists / maps).
pub struct RutValue(pub Value);

/// The per-instance resource budget. Phase-1 defaults; tunable per embedder.
pub fn default_limits() -> rut_vm::interp::Limits {
    rut_vm::interp::Limits {
        fuel: Some(50_000_000),
        heap_limit_bytes: Some(64 * 1024 * 1024),
        interrupt_every: 1024,
    }
}

// ---------------------------------------------------------------------------
// The `tur_host` package — decl rows (mounted in-memory as a Module) +
// bodies (a HostPkg installed into the per-instance HostRegistry).
//
// MECHANISM ONLY: mount + the C8 no-mount trap and the `rs_*` store rows.
// Element rows live in the plugins that own
// their view types and arrive through the [`RutPkgExt`] seam.
// ---------------------------------------------------------------------------

/// The in-memory `tur_host` pkg (the DECL side): the mechanism surface
/// rut code compiles against. Offered to the run chain — no filesystem
/// involved. Plugin families extend it through their pushed
/// [`RutPkgExt`]s (decl rows + consts).
pub fn tur_decl_pkg() -> rut_driver::Pkg {
    let row = |name: &str, params: Vec<TypeId>, ret: TypeId| (name.to_string(), params, ret, false);
    let host_funcs: Vec<(String, Vec<TypeId>, TypeId, bool)> = vec![
        // stash the root — the engine applies it after `start` returns
        row("mount", vec![TY_OPAQUE], TY_NIL),
        // The kit's twin spelling (see the mount bodies below): same
        // decl, same body — the kit's own `mount` wrapper shadows the
        // row name, so its body calls this alias.
        row("mount_raw", vec![TY_OPAQUE], TY_NIL),
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
        // C8 — the watch rail (the guarded flush-time delivery; the
        // callback is the kit-sealed `Mutation<nil>` composition atom).
        // The str-typed derive rows are gone — the kit's `derive<T>`
        // mints through `derive_seal`'s boxed lane.
        row("rs_watch_mut", vec![TY_U64, TY_U64], TY_OPAQUE),
        row("rs_watch_start", vec![TY_OPAQUE], TY_NIL),
        row("rs_watch_stop", vec![TY_OPAQUE], TY_NIL),
        // the boxed rail — the GENERIC source lane: an erased rut value
        // crosses as an opaque box, the engine decodes it into the same
        // native KV variant the typed rows write (the box's payload kind
        // picks the variant; `rs_box_stamp` reports the number lane so
        // the read can re-stamp u64 vs f64 boxes losslessly).
        row("rs_source_box", vec![TY_OPAQUE], TY_U64),
        row("rs_box_stamp", vec![TY_OPAQUE], TY_U64),
        row("ctx_get_box", vec![TY_OPAQUE, TY_U64, TY_U64], TY_OPAQUE),
        row("ctx_set_box", vec![TY_OPAQUE, TY_U64, TY_OPAQUE], TY_NIL),
        row("ctx_run_box", vec![TY_OPAQUE, TY_U64, TY_OPAQUE], TY_NIL),
        // M1 — the mutation rail: the ctx read/write/compose rows + the
        // mutation/derive sealers (the write-side twin of the C8 face).
        row("ctx_bridge", vec![], TY_OPAQUE),
        row("ctx_get_f64", vec![TY_OPAQUE, TY_U64], TY_F64),
        row("ctx_get_str", vec![TY_OPAQUE, TY_U64], TY_STR),
        row("ctx_get_bool", vec![TY_OPAQUE, TY_U64], TY_BOOL),
        row("ctx_get_value", vec![TY_OPAQUE, TY_U64], TY_OPAQUE),
        row("ctx_get_opaque", vec![TY_OPAQUE, TY_U64], TY_OPAQUE),
        row("ctx_set_f64", vec![TY_OPAQUE, TY_U64, TY_F64], TY_NIL),
        row("ctx_set_str", vec![TY_OPAQUE, TY_U64, TY_STR], TY_NIL),
        row("ctx_set_bool", vec![TY_OPAQUE, TY_U64, TY_BOOL], TY_NIL),
        row("ctx_set_value", vec![TY_OPAQUE, TY_U64, TY_OPAQUE], TY_NIL),
        row("ctx_set_opaque", vec![TY_OPAQUE, TY_U64, TY_OPAQUE], TY_NIL),
        row("ctx_run_nil", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("ctx_run_f64", vec![TY_OPAQUE, TY_U64, TY_F64], TY_NIL),
        row("mutate_seal", vec![TY_OPAQUE, TY_U64], TY_U64),
        row("derive_seal", vec![TY_OPAQUE, TY_U64], TY_U64),
        row("str_parse_f64", vec![TY_STR], TY_F64),
        // pure math (f64 radians in/out) — the orbit / wave case shapes
        row("math_sin", vec![TY_F64], TY_F64),
        row("math_cos", vec![TY_F64], TY_F64),
        // The kit wrappers' twin spellings (the `mount_raw` pattern): the
        // kit defines its own `math_sin` / `math_cos` / `str_parse_f64`
        // (the authored surface's names), which shadow the row names —
        // the wrapper bodies reach the rows through these aliases.
        row("str_parse_f64_raw", vec![TY_STR], TY_F64),
        row("math_sin_raw", vec![TY_F64], TY_F64),
        row("math_cos_raw", vec![TY_F64], TY_F64),
    ];
    // C6 — async capabilities (clipboard + bytes helpers; the async rows
    // ride the driver's five-row family expansion).
    let mut funcs = host_funcs;
    funcs.extend(async_caps::decl_rows());
    rut_driver::Pkg {
        spec: "tur_host".to_string(),
        namespace: Some("tur_host".to_string()),
        body: PkgBody::Host {
            host_funcs: funcs,
            consts: Vec::new(),
            native_types: Vec::new(),
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
    let mut pkg = rut_vm::interp::HostPkg::new("tur_host");

    // ---- mount + the C8 no-mount law ------------------------------------
    //
    // A face-driven call (a derive / item builder materializing mid-flush)
    // may NOT re-mount — the trap is reported through the error rail and
    // the flush continues. `mount_raw` is the kit's twin spelling of the
    // SAME body: the kit defines its own `pub fn mount(v: View)` (the
    // unwrap happens kit-side), which shadows the row's name — the body
    // reaches the row through this alias.
    fn mount_body(
        _vm: &mut rut_vm::interp::Vm,
        h: Rc<RutHandles>,
        view: Opaque<RutView>,
    ) -> Result<(), rut_vm::Trap> {
        if h.face_busy.get() > 0 {
            return Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                "tur_host::mount inside a face call (derive / item builder) —                  mounting is a start-time or intent-drain-time op only",
            ));
        }
        let root = view.with(|v| v.0.clone())?;
        *h.pending_root.borrow_mut() = Some(root);
        Ok(())
    }
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mount", (Opaque<RutView>,) -> (), move |vm: &mut rut_vm::interp::Vm, view: Opaque<RutView>| {
        mount_body(vm, h.clone(), view)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mount_raw", (Opaque<RutView>,) -> (), move |vm: &mut rut_vm::interp::Vm, view: Opaque<RutView>| {
        mount_body(vm, h.clone(), view)
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

    // ---- the boxed rail (the GENERIC source lane) -------------------------
    //
    // `source<T>(v)` boxes v (`opaque(v)`) and mints through here: the
    // engine opens the box (`rut_box_payload` — the one read path host
    // code has into a rut seal box) and stores the SAME native KV variant
    // the typed rows write, so every existing engine-side read (bound
    // props, `ScalarRead`, the structured rails) keeps working unchanged.
    // Host boxes decode too: a `RutValue` (the structured lane) unwraps
    // to its native value, any other host box rides the opaque lane
    // (the controller-slot shape).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_box", (OpaqueRef,) -> u64, move |vm: &mut rut_vm::interp::Vm, v: OpaqueRef| {
        let value = boxed_to_value(vm, &v)?;
        let s: Source<Value> = h.store.bridge().decl_source(value);
        Ok(s.id().0 as u64)
    });
    // The number lane's stamp (the one KV collapse): `1` — the box held
    // an f64 (reads re-stamp f64), `2` — a u64 / i64 (reads re-stamp the
    // integer width), `0` — not a number (the read stamps by KV kind).
    rut_vm::pkg_fn!(pkg, "rs_box_stamp", (OpaqueRef,) -> u64, move |vm: &mut rut_vm::interp::Vm, v: OpaqueRef| {
        Ok(match rut_vm::rut_box_payload(vm, &v) {
            Ok((_, rut_vm::Value::F64(_))) => STAMP_NUM_F64,
            Ok((_, rut_vm::Value::I64(_))) => STAMP_NUM_U64,
            _ => 0,
        })
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

    // ---- parse helper -------------------------------------------------------
    // str -> f64 parse (0 on failure) — the edit-field confirm path.
    rut_vm::pkg_fn!(pkg, "str_parse_f64", (&str,) -> f64, |_vm: &mut rut_vm::interp::Vm, s: &str| {
        Ok(s.trim().parse::<f64>().unwrap_or(0.0))
    });
    // Trig helpers (f64 radians in/out) — the pure-math rows. No instance
    // state, no reactivity: a case derives motion from them (the orbit /
    // wave shapes feed them a progress atom and format the result).
    rut_vm::pkg_fn!(pkg, "math_sin", (f64,) -> f64, |_vm: &mut rut_vm::interp::Vm, x: f64| {
        Ok(x.sin())
    });
    rut_vm::pkg_fn!(pkg, "math_cos", (f64,) -> f64, |_vm: &mut rut_vm::interp::Vm, x: f64| {
        Ok(x.cos())
    });
    // The kit wrappers' twin spellings — SAME bodies (see the decl note).
    rut_vm::pkg_fn!(pkg, "str_parse_f64_raw", (&str,) -> f64, |_vm: &mut rut_vm::interp::Vm, s: &str| {
        Ok(s.trim().parse::<f64>().unwrap_or(0.0))
    });
    rut_vm::pkg_fn!(pkg, "math_sin_raw", (f64,) -> f64, |_vm: &mut rut_vm::interp::Vm, x: f64| {
        Ok(x.sin())
    });
    rut_vm::pkg_fn!(pkg, "math_cos_raw", (f64,) -> f64, |_vm: &mut rut_vm::interp::Vm, x: f64| {
        Ok(x.cos())
    });

    // C6 — async capabilities (clipboard + the bytes helpers).
    async_caps::install(&mut pkg, handles);
    // C8 — derived atoms + watch (the guarded flush-time VM call).
    derive::install(&mut pkg, handles);
    // M1 — the mutation rail (the ctx rows + the sealers).
    mutation::install(&mut pkg, handles);

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
    /// The instance's frame self-drive — `InstanceContext::request_frame`
    /// as a plain fn (rows live in plugin crates; they get the behavior
    /// without the context type). A paint-worthy state change raised
    /// outside a flush (a programmatic controller write) re-arms an idle
    /// worker; during a flush it is the no-op the context documents.
    pub request_frame: Rc<dyn Fn()>,
    /// The instance-owned tree handle — `apply_root` builds into it.
    pub element_tree: crate::core::elements::NodeTree,
    /// The instance's focus manager — the `focus_request` row targets it
    /// (the FocusChange flush pushes the focus/blur mutations next frame).
    pub focus_manager: Rc<std::cell::RefCell<crate::core::focus::FocusManager>>,
    /// The root stashed by `tur_host::mount` during `start`, applied by the
    /// engine after the call returns (outside the VM, on the mount path).
    pub pending_root: std::cell::RefCell<Option<Rc<dyn View>>>,
    /// Callback intents queued by element callbacks (the row closures are
    /// realm-free: they only push here). Drained at pump level after
    /// flush — `(sealed callback box, id, payload)`.
    pub pending_calls: std::cell::RefCell<Vec<Intent>>,
    /// Monotonic click counter stamped into click intents.
    pub click_seq: std::cell::Cell<u64>,
    /// The worker→host channel — runtime-error reports for face traps ride
    /// the same `RuntimeError` message the JS rail used.
    pub host_tx: crate::core::app::HostTx,
    /// The engine's shared clock — the animation rows' `now_ms` source.
    pub clock: Rc<dyn crate::core::clock::Clock>,
    /// The engine-wide mutation queue — animation `onTick` callbacks ride
    /// it (same dispatch path the JS controllers used).
    pub mutation_queue:
        Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>,
    /// The instance context — capability lookups + worker-side spawns (the
    /// async capability rows: clipboard / net / filepicker).
    pub inst: InstanceContext,
    /// The flush-time VM face: view factories / deriveds minted
    /// by rows reach the VM through it. Detached until boot installs the
    /// VM; guards (depth, no-mount) live here.
    pub face: Rc<VmFace>,
    /// Above zero while a face-driven VM call is in flight — `tur_host::mount`
    /// traps inside one (the C8 no-mount law: a derive/build that tries to
    /// re-mount the tree can never wedge the frame).
    pub face_busy: std::cell::Cell<u32>,
}

/// One queued callback intent — the payload the drain dispatches into an
/// infra dispatch entry. The callback ITSELF is a rut fn value the kit
/// sealed into an opaque box (`opaque(cb)`) at registration; only the box
/// (a handle — the boundary's one-cell law) and the payload cross. The
/// dispatch entry lives in the kit that sealed the callback (the
/// scope law — see the kit's dispatch-entry docs); `entry` names it (an
/// engine constant, never an author string). The legacy click shape plus
/// the record payloads (keys, pointer positions, raw values) that the
/// gesture / animation / watch rails queue.
#[derive(Clone, Debug)]
pub enum Intent {
    /// The tap shape: `(cb, id_a, id_b, seq)`.
    Click {
        entry: &'static str,
        cb: OpaqueRef,
        a: u64,
        b: u64,
        seq: f64,
    },
    /// A raw value payload (`watch(atom, cb)` change deliveries).
    Value {
        entry: &'static str,
        cb: OpaqueRef,
        a: u64,
        value: crate::core::edgy::Value,
    },
}

impl Intent {
    /// The intent's dispatch-entry name (the drain's fire target; also
    /// the drain-log handle).
    pub fn entry(&self) -> &'static str {
        match self {
            Intent::Click { entry, .. } | Intent::Value { entry, .. } => entry,
        }
    }
}

/// The infra dispatch-entry names (engine constants - the kit modules
/// declare the matching `entry fn`s; see the kit's scope-law note). The
/// `tur_kit` prelude owns these shapes; other kits declare their own.
/// (The M2 sweep retired the legacy pointer/key shapes — the gesture,
/// input, focus, mouse-region and lifecycle pads store sealed mutations
/// now. The watch rail re-railed onto `rs_watch_mut` + the MNIL lane, so
/// the lifecycle `before_destroy` fn rail is the click shape's LAST user.)
pub mod cb_entries {
    /// The `(cb, a, b, n)` click shape - `before_destroy` (its last user
    /// since the watch rail moved to `rs_watch_mut` + `__tur_cb_mnil`).
    pub const CLICK: &str = "__tur_cb_click";
    /// The `(cb) -> view` zero-arg branch-builder shape (Condition
    /// then/else + Switch cases — invoked at activation).
    pub const BUILD0: &str = "__tur_cb_build0";
    /// The `(cb, i) -> view` lazy/table-header builder shape.
    pub const BUILD1: &str = "__tur_cb_build1";
    /// The `(cb, i, item) -> view` Each-builder shape.
    pub const BUILD2: &str = "__tur_cb_build2";
    /// The `(cb, i, col) -> view` table-row-builder shape.
    pub const BUILD2I: &str = "__tur_cb_build2i";
    /// The `(cb, v) -> str` derive-format shape.
    pub const STR1: &str = "__tur_cb_str1";
    /// The `(cb, a, b) -> str` two-dep derive-format shape.
    pub const STR2: &str = "__tur_cb_str2";
}

/// The mutation rail's dispatch-entry names (M1) — kit-owned shapes, same
/// scope law as [`cb_entries`]: the kit (`tur_kit.rut`) declares the
/// matching `entry fn`s that construct the ctx + event and call the
/// user's typed fn.
pub mod mutation_entries {
    /// The nil-arg mutation: `(cb, h)` — the click surface.
    pub const MNIL: &str = "__tur_cb_mnil";
    /// The pointer mutation: `(cb, h, lx, ly, gx, gy, btn)` — the entry
    /// constructs the typed `PointerEvent`.
    pub const MPTR: &str = "__tur_cb_mptr";
    /// The typed-arg mutation: `(cb, h, a)` — ticks and `ctx.run_f64`.
    pub const MF64: &str = "__tur_cb_mf64";
    /// The region enter/exit mutations: `(cb, h, lx, ly, gx, gy)` — the
    /// entries construct `EnterEvent` / `ExitEvent`.
    pub const MENTER: &str = "__tur_cb_menter";
    pub const MEXIT: &str = "__tur_cb_mexit";
    /// The key mutation: `(cb, h, key, code, modifiers)`.
    pub const MKEY: &str = "__tur_cb_mkey";
    /// The input mutation: `(cb, h, value, enter)`.
    pub const MINPUT: &str = "__tur_cb_minput";
    /// The payload-less events: `(cb, h)` — the entries construct
    /// `FocusEvent` / `BlurEvent` / `MountEvent`.
    pub const MFOCUS: &str = "__tur_cb_mfocus";
    pub const MBLUR: &str = "__tur_cb_mblur";
    pub const MMOUNT: &str = "__tur_cb_mmount";
    /// The bytes mutation: `(cb, h, data)` — net-stream chunks.
    pub const MBYTES: &str = "__tur_cb_mbytes";
    /// The boxed composition mutation: `(cb, h, box)` — the generic
    /// lane; the entry hands the box to the kit's fixed-type wrapper.
    pub const MBOX: &str = "__tur_cb_mbox";
    /// The derive format fns: `(cb, h) -> T` — the entry constructs the
    /// read-only `DeriveCtx`.
    pub const DERIVE_F64: &str = "__tur_cb_derive_f64";
    pub const DERIVE_STR: &str = "__tur_cb_derive_str";
    pub const DERIVE_BOOL: &str = "__tur_cb_derive_bool";
    /// The boxed derive: `(cb, h) -> box` — the generic lane (the kit's
    /// wrapper returns the user fn's result boxed).
    pub const DERIVE_BOX: &str = "__tur_cb_derive_box";
    /// The box kind probes + typed unboxes the boxed derive drives (the
    /// drain closure holds no VM, so the unbox runs rut-side).
    pub const DERIVE_KIND: &str = "__tur_cb_derive_kind";
    pub const UNBOX_F64: &str = "__tur_cb_unbox_f64";
    pub const UNBOX_STR: &str = "__tur_cb_unbox_str";
    pub const UNBOX_BOOL: &str = "__tur_cb_unbox_bool";
    pub const UNBOX_U64: &str = "__tur_cb_unbox_u64";
    /// The box kind probe's answers (`__tur_cb_derive_kind`).
    pub const BOX_KIND_NIL: u64 = 0;
    pub const BOX_KIND_F64: u64 = 1;
    pub const BOX_KIND_STR: u64 = 2;
    pub const BOX_KIND_BOOL: u64 = 3;
    pub const BOX_KIND_U64: u64 = 4;
}

/// The seal tags — the row→entry selectors the sealers bake into their
/// closures. The kit spells the same numbers as its own consts (the kit
/// cannot name Rust items; one law, two spellings).
pub mod seal_tags {
    /// `mutate_seal`: the nil-arg mutation (clicks).
    pub const MUT_NIL: u64 = 0;
    /// `mutate_seal`: the pointer-event mutation (drags, context menus).
    pub const MUT_PTR: u64 = 1;
    /// `mutate_seal`: the typed-arg mutation (ticks, `ctx.run_f64`).
    pub const MUT_F64: u64 = 2;
    /// `mutate_seal`: the region enter/exit mutations.
    pub const MUT_ENTER: u64 = 3;
    pub const MUT_EXIT: u64 = 4;
    /// `mutate_seal`: the key event mutation.
    pub const MUT_KEY: u64 = 5;
    /// `mutate_seal`: the input event mutation.
    pub const MUT_INPUT: u64 = 6;
    /// `mutate_seal`: the payload-less event mutations.
    pub const MUT_FOCUS: u64 = 7;
    pub const MUT_BLUR: u64 = 8;
    pub const MUT_MOUNT: u64 = 9;
    /// `mutate_seal`: the bytes mutation (net-stream chunks).
    pub const MUT_BYTES: u64 = 10;
    /// `mutate_seal`: the boxed composition mutation — the GENERIC lane
    /// (`ctx.run<A>`'s crossing; the kit's fixed-type wrapper recovers A).
    pub const MUT_BOX: u64 = 11;
    /// `derive_seal` tags: the value kind (the entry's return type).
    pub const DRV_F64: u64 = 0;
    pub const DRV_STR: u64 = 1;
    pub const DRV_BOOL: u64 = 2;
    /// `derive_seal`: the boxed derive — the GENERIC lane (the kit's
    /// wrapper returns the user fn's result boxed; the kit's probe
    /// entries recover kind + value).
    pub const DRV_BOX: u64 = 3;
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
///   calling `tur_host::mount` inside traps (checked by the `mount` row).
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
        let vm = self.vm.borrow().clone().upgrade().ok_or_else(|| {
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
                            Err(t2)
                                if t2.kind == rut_vm::TrapKind::OutOfFuel
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
                            &format!(
                                "face call `{name}`: out of fuel after bounded grants (total used {})",
                                guard.fuel_used
                            ),
                        );
                        let _ = t;
                    }
                    done
                }
                Err(t) => {
                    report_runtime_error(
                        handles,
                        &format!("face call `{name}`: {} — {}", t.name(), t.msg),
                    );
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
    /// `entry fn start()`'s answer — the u64 answer when declared `-> u64`,
    /// or the answered-context slot token when declared `-> opaque` (the
    /// `tur_start_answer` channel extended to opaques; see
    /// [`Self::entry_slot_mint`]). 0 when `start` returns nil.
    pub start_answer: u64,
    /// The embedder-facing answered-context slots (the entry RPC's opaque
    /// pass-through — the stash's old cross-entry lifetime, now held by
    /// the ENGINE instead of the module): boot's opaque `start` answer and
    /// the `call_rut_entry_opaque` lane mint tokens here, and control
    /// entries take them back by token. Dropped with the module — teardown
    /// clears (the same lifetime the stash had, embedder-facing).
    entry_slots: std::cell::RefCell<std::collections::HashMap<u64, OpaqueRef>>,
    entry_slot_next: std::cell::Cell<u64>,
}

impl RutRuntime {
    /// Convert an opaque handle minted by a rut row (`RutView`) back into
    /// the materialized view — the item-builder face's crossing back from
    /// an `entry fn(index) -> opaque`.
    pub fn view_of(handle: &OpaqueRef) -> Option<Rc<dyn View>> {
        let v = Opaque::<RutView>::from_handle(handle).ok()?;
        v.with(|v| Some(v.0.clone())).ok()?
    }
    /// Assemble a fresh session (core + the in-memory `tur_host` decl pkg +
    /// every extension's prelude modules) and compile `source` against it.
    /// Split from [`Self::boot`] so a syntactically-broken module fails
    /// BEFORE any teardown runs (the parse-first contract).
    fn compile(
        source: &str,
        exts: &[RutPkgExt],
    ) -> Result<
        (
            Rc<rut_core::binary::Program>,
            rut_vm::interp::HostPkgContext,
        ),
        String,
    > {
        // The decl surface: the engine rows plus every extension's rows
        // (plugin-owned — the element families + capability crates), so
        // the compile sees the full surface the boot will bind. Extensions
        // also contribute prelude modules (the kit — the authored builder
        // surface, owned by the standard bundle assembly, never by core).
        let mut ext_decl: Vec<(String, Vec<TypeId>, TypeId, bool)> = Vec::new();
        let mut ext_consts: Vec<(String, TypeId, u64)> = Vec::new();
        let mut preludes: Vec<rut_driver::Pkg> = Vec::new();
        let mut probe = rut_vm::interp::HostPkg::new("tur_host");
        for ext in exts {
            ext(&mut RutPkgCx {
                decl: &mut ext_decl,
                consts: &mut ext_consts,
                pkg: &mut probe,
                handles: None,
                preludes: &mut preludes,
            });
        }
        let mut tur_pkg = tur_decl_pkg();
        if let PkgBody::Host {
            host_funcs, consts, ..
        } = &mut tur_pkg.body
        {
            host_funcs.extend(ext_decl);
            consts.extend(ext_consts);
        }
        // The async weave (the Future machinery + the launch/sleep rows) —
        // offered to EVERY target: the decl surface lowers from the vendored
        // upstream `.d.rut` (wasm has no filesystem — the old disk-bound
        // `mount_std_async` gate died with the driver's path-free rewrite),
        // and the bodies install at boot (`rut_std::async_host::pkg()` —
        // pure VM-driving rows: launch / cancel / arm_timer).
        let async_decl_txt: &str = include_str!("async_host.d.rut");
        let mut async_host = rut_driver::lower_decl_module(async_decl_txt, "async_host.d.rut")
            .map_err(|e| format!("mount async_host decl: {e}"))?;
        async_host.spec = "async_host".to_string();
        // The typed launcher surface (`launch_future` / `sleep` / the
        // competition rows) — upstream's `futures` inline package, rut
        // source over the `__*` engine rows (vendored alongside).
        let futures = rut_driver::Pkg::source("futures", include_str!("futures.rut"));
        // The run chain: offer the pkgs (first-offer-wins), root at the app
        // source. `core` is auto-offered by `.compile()`; the host-row
        // snapshot for the boot-side installs comes from the same offer set
        // (core carries no host rows, so the snapshot matches the old
        // session's `host_pkg_context`).
        let mut offered = vec![tur_pkg.clone(), async_host.clone()];
        offered.extend(preludes.iter().cloned());
        let mut run = rut_driver::RutRun::new()
            .pkg(tur_pkg)
            .pkg(async_host)
            .pkg(futures)
            .pkg(rut_driver::Pkg::source("app", source))
            .entrypoint("app");
        for pkg in preludes {
            run = run.pkg(pkg);
        }
        let compiled = run.compile().map_err(|e| e.msg)?;
        if !compiled.graph.diags.is_empty() {
            let msgs: Vec<String> = compiled.graph.diags.iter().map(|d| d.msg.clone()).collect();
            return Err(msgs.join("; "));
        }
        let prog = compiled
            .graph
            .program
            .ok_or("rut compile emitted no binary")?;
        rut_vm::verify::verify(&prog).map_err(|e| format!("verify: {e}"))?;
        Ok((Rc::new(prog), rut_driver::host_pkg_ctx(&offered)))
    }

    /// Parse + compile only (the parse-first half of the load contract) —
    /// a broken reload must fail before any teardown runs.
    pub fn parse_check(
        source: &str,
        exts: &[RutPkgExt],
    ) -> Result<(), crate::core::app::ModuleError> {
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
            // The frame self-drive: a clone of the worker context whose
            // `request_frame` sets the paint flag + wakes an idle worker.
            request_frame: {
                let ctx = js_ctx.clone();
                Rc::new(move || ctx.request_frame())
            },
            element_tree: js_ctx.element_tree.clone(),
            focus_manager: js_ctx.focus_manager.clone(),
            pending_root: std::cell::RefCell::new(None),
            pending_calls: std::cell::RefCell::new(Vec::new()),
            click_seq: std::cell::Cell::new(0),
            host_tx: js_ctx.host_tx.clone(),
            clock: inputs.clock,
            mutation_queue: js_ctx.mutation_queue.clone(),
            inst: js_ctx.clone(),
            face,
            face_busy: std::cell::Cell::new(0),
        });

        let mut hosts = rut_vm::interp::HostRegistry::new();
        // The async launcher set (`__launch` / `__abort` / `__sleep`) —
        // the vendored `async_host` decl demands these bodies (both
        // targets: the rows are pure VM-driving — launch / cancel /
        // arm_timer — and the decl lowers from the vendored `.d.rut`).
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
        // `entry fn start()`'s declared answer type drives the boot call:
        // `-> u64` records the answer, `-> opaque` answers the module's
        // AppContext (the context-crossing contract's eager shape — a slot
        // token becomes the start answer), `-> nil` answers nothing. A
        // module with no `start` export but an `entry_start` boots LAZILY —
        // the embedder's first `call_rut_entry_opaque("entry_start")` runs
        // it (the fixture contract's `fn start() -> AppContext` shape).
        let start_ret = export_of("start")
            .and_then(|fid| prog.funcs.get(fid))
            .map(|f| f.ret);
        let has_entry_start = export_of("entry_start").is_some();

        let vm = rut_vm::interp::Vm::builder()
            .program(prog)
            .limits(default_limits())
            .hooks(rut_vm::interp::HostHooks::default())
            .hosts(hosts)
            .build()
            .map_err(|e| crate::core::app::ModuleError::Eval(format!("boot: {}", e.msg)))?;
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
            entry_slots: std::cell::RefCell::new(std::collections::HashMap::new()),
            entry_slot_next: std::cell::Cell::new(1),
        };
        rt.call_start(start_ret, has_entry_start)?;
        Ok(rt)
    }

    fn call_start(
        &mut self,
        start_ret: Option<TypeId>,
        has_entry_start: bool,
    ) -> Result<(), crate::core::app::ModuleError> {
        let mut vm = self.vm.borrow_mut();
        match start_ret {
            Some(TY_U64) => {
                let answer = vm.call::<_, u64>("start", ()).map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!(
                        "start: {} — {}",
                        t.name(),
                        t.msg
                    ))
                })?;
                self.start_answer = answer;
            }
            // The eager context-crossing shape: `entry fn start() ->
            // opaque` answers the module's AppContext — the engine slots
            // it and the token IS the start answer (the embedder passes it
            // back through the cx entry lanes).
            Some(TY_OPAQUE) => {
                let cx = vm.call::<_, OpaqueRef>("start", ()).map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!(
                        "start: {} — {}",
                        t.name(),
                        t.msg
                    ))
                })?;
                self.start_answer = self.entry_slot_mint(cx);
            }
            Some(TY_NIL) => {
                vm.call::<_, ()>("start", ()).map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!(
                        "start: {} — {}",
                        t.name(),
                        t.msg
                    ))
                })?;
            }
            Some(other) => {
                return Err(crate::core::app::ModuleError::Eval(format!(
                    "start: unsupported answer type (type id {other})"
                )));
            }
            None => {
                if !has_entry_start {
                    return Err(crate::core::app::ModuleError::Eval(
                        "boot: the module has no `entry fn start()`".into(),
                    ));
                }
                // The lazy fixture shape — boot defers to the embedder's
                // first `entry_start` probe.
            }
        }
        Ok(())
    }

    /// Call a named `entry fn(u64, f64)` — the engine→rut event rail
    /// (input dispatch, embedder events). Runs OUTSIDE flush; rows must
    /// not mount (stash-and-apply is a `start`-time contract in Phase 2).
    pub fn call_entry(
        &mut self,
        name: &str,
        a: u64,
        b: f64,
    ) -> Result<(), crate::core::app::ModuleError> {
        self.vm
            .borrow_mut()
            .call::<_, ()>(name, (a, b))
            .map(|_| ())
            .map_err(|t| {
                crate::core::app::ModuleError::Eval(format!("{name}: {} — {}", t.name(), t.msg))
            })
    }

    /// Whether the module exports `name` (the legacy rail's optional-entry
    /// check; the cx lanes treat a missing control entry as an error).
    pub fn has_export(&self, name: &str) -> bool {
        let vm = self.vm.borrow();
        vm.prog
            .exports
            .iter()
            .any(|(n, _)| vm.prog.interner.name(*n) == name)
    }

    /// The declared return type of a named export (None when absent) —
    /// the cx lane's answer-decode key.
    pub fn export_ret(&self, name: &str) -> Option<TypeId> {
        let vm = self.vm.borrow();
        let (_, fid) = vm
            .prog
            .exports
            .iter()
            .find(|(n, _)| vm.prog.interner.name(*n) == name)?;
        vm.prog.funcs.get(*fid as usize).map(|f| f.ret)
    }

    /// Mint a slot over an answered opaque — the entry RPC's answer
    /// channel (boot's opaque `start` answer; the
    /// `call_rut_entry_opaque` lane's answers). The worker holds the
    /// handle here; the token is the embedder's only view.
    fn entry_slot_mint(&self, o: OpaqueRef) -> u64 {
        let token = self.entry_slot_next.get();
        self.entry_slot_next.set(token + 1);
        self.entry_slots.borrow_mut().insert(token, o);
        token
    }

    /// Clone a held slot's handle — the cx crossing back INTO the VM (the
    /// slot STAYS: one context re-crosses on every control call).
    pub fn entry_slot_handle(&self, token: u64) -> Result<OpaqueRef, crate::core::app::ModuleError> {
        self.entry_slots
            .borrow()
            .get(&token)
            .cloned()
            .ok_or_else(|| {
                crate::core::app::ModuleError::Eval(format!(
                    "call_rut_entry: no context held for slot {token} (module reloaded over it?)"
                ))
            })
    }

    /// The `entry fn() -> opaque` probe — the fixture contract's lazy
    /// `entry_start` boot. Mints a slot over the answer and reports the
    /// token.
    pub fn call_entry_opaque(
        &mut self,
        name: &str,
    ) -> Result<crate::core::app::RutEntryAnswer, crate::core::app::ModuleError> {
        let o = self
            .vm
            .borrow_mut()
            .call::<_, OpaqueRef>(name, ())
            .map_err(|t| {
                crate::core::app::ModuleError::Eval(format!(
                    "{name}: {} — {}",
                    t.name(),
                    t.msg
                ))
            })?;
        Ok(crate::core::app::RutEntryAnswer::Opaque(
            self.entry_slot_mint(o),
        ))
    }

    /// `entry fn(opaque)` — the held context only; the answer decodes
    /// under the export's declared return.
    pub fn call_entry_cx(
        &mut self,
        name: &str,
        cx: &OpaqueRef,
    ) -> Result<crate::core::app::RutEntryAnswer, crate::core::app::ModuleError> {
        self.entry_cx_call(name, (cx.clone(),))
    }

    /// `entry fn(opaque, u64)` — the context + a scalar.
    pub fn call_entry_cx_u64(
        &mut self,
        name: &str,
        cx: &OpaqueRef,
        a: u64,
    ) -> Result<crate::core::app::RutEntryAnswer, crate::core::app::ModuleError> {
        self.entry_cx_call(name, (cx.clone(), a))
    }

    /// `entry fn(opaque, f64)` — the context + a float.
    pub fn call_entry_cx_f64(
        &mut self,
        name: &str,
        cx: &OpaqueRef,
        b: f64,
    ) -> Result<crate::core::app::RutEntryAnswer, crate::core::app::ModuleError> {
        self.entry_cx_call(name, (cx.clone(), b))
    }

    /// The shared cx-lane body: ONE vm.call with the given args tuple, the
    /// answer decoded under the export's declared return type (nil / u64 /
    /// f64 / str / opaque — anything else is a loud error naming both
    /// sides). An opaque answer slots and reports its token.
    fn entry_cx_call<A: CallArgs>(
        &mut self,
        name: &str,
        args: A,
    ) -> Result<crate::core::app::RutEntryAnswer, crate::core::app::ModuleError> {
        type Ans = crate::core::app::RutEntryAnswer;
        let ret = self.export_ret(name).ok_or_else(|| {
            crate::core::app::ModuleError::Eval(format!(
                "call_rut_entry: no export `{name}` (a control entry must exist)"
            ))
        })?;
        let answer = match ret {
            TY_NIL => self
                .vm
                .borrow_mut()
                .call::<A, ()>(name, args)
                .map(|_| Ans::Nil),
            TY_U64 => self
                .vm
                .borrow_mut()
                .call::<A, u64>(name, args)
                .map(Ans::U64),
            TY_F64 => self
                .vm
                .borrow_mut()
                .call::<A, f64>(name, args)
                .map(Ans::F64),
            TY_STR => self
                .vm
                .borrow_mut()
                .call::<A, String>(name, args)
                .map(Ans::Str),
            TY_OPAQUE => self
                .vm
                .borrow_mut()
                .call::<A, OpaqueRef>(name, args)
                .map(|o| Ans::Opaque(self.entry_slot_mint(o))),
            other => Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                format!("call_rut_entry: `{name}` answers an unsupported type (type id {other})"),
            )),
        };
        answer.map_err(|t| {
            crate::core::app::ModuleError::Eval(format!("{name}: {} — {}", t.name(), t.msg))
        })
    }

    /// Apply the root stashed by `tur_host::mount` into the instance tree —
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
            let _ = self.handles.host_tx.unbounded_send(HostMsg::RuntimeError {
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
            let entry = intent.entry();
            if let Err(t) = self.call_intent(intent) {
                // The audit trail: a failed callback surfaces through the
                // error rail's log face (the embedder sees the report; the
                // rest of the drain continues).
                tracing::error!("rut callback {entry} trap: {} — {}", t.name(), t.msg);
            }
        }
        calls.len()
    }

    /// Dispatch one intent into its infra dispatch entry (per-shape
    /// signatures; the sealed callback box is the first argument - the
    /// kit-scope entry recovers the fn with `opaque.downcast` and calls
    /// it).
    fn call_intent(&mut self, intent: &Intent) -> Result<(), rut_vm::Trap> {
        let mut vm = self.vm.borrow_mut();
        match intent {
            Intent::Click {
                entry,
                cb,
                a,
                b,
                seq,
            } => vm.call::<_, ()>(entry, (cb.clone(), *a, *b, *seq)),
            Intent::Value {
                entry,
                cb,
                a,
                value,
            } => {
                let n = match value {
                    Value::Num(n) => *n,
                    Value::Bool(b) => *b as u64 as f64,
                    _ => 0.0,
                };
                vm.call::<_, ()>(entry, (cb.clone(), *a, n))
            }
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

/// The boot wiring `WorkerBackend::load_rut_module_inner` hands to
/// [`RutRuntime::boot`] — the engine clock (the rows' `now_ms` source; the
/// same `Clock` the animation subsystem ticks with).
pub struct RutRealmInputs {
    pub clock: std::rc::Rc<dyn crate::core::clock::Clock>,
}

/// The pkg-extension context an installer sees: the `tur_host` pkg's decl
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
    /// Prelude rut pkgs registered into the compile run before the app
    /// source compiles (the kit — the authored builder surface). Core
    /// never fills this; the standard bundle assembly does.
    pub preludes: &'a mut Vec<rut_driver::Pkg>,
}

/// A rut pkg extension: plugin-owned rows for the `tur_host` pkg (e.g.
/// tur-animation's C5 rows, each element family's spec + rows). Plugins
/// push one during `register`; both the compile (decl) and boot (bodies)
/// phases drain them.
pub type RutPkgExt = Rc<dyn Fn(&mut RutPkgCx<'_>)>;
