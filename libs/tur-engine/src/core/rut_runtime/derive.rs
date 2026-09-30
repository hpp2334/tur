//! C8 — derived atoms + `watch` on the rut rail: the decided law — a
//! **synchronous VM call during flush**, guarded.
//!
//! `rs_derive(name, dep)` mints a derived whose materialization reads its
//! dep through the tracked read face (auto-dependency tracking works
//! exactly as for JS derives) and then calls `entry fn name(dep: f64)`
//! through the [`VmFace`]. The face applies every guard the plan decided:
//!
//! - **fuel-capped** — the call runs on the VM's budget with bounded
//!   retry grants, then one drain grant that always returns the machine
//!   to idle (never parked mid-flush);
//! - **no-mount** — `face_busy` is raised for the call's duration; a
//!   derive that tries `tur::mount` traps (checked by the mount row);
//! - **depth-limited** — nested face calls cap at `VM_FACE_MAX_DEPTH`;
//! - **traps never abort the flush** — reported through the
//!   runtime-error rail, the derived falls back to `Value::Nil`.
//!
//! `rs_watch(atom, cb)` rides the sanctioned `register_watch` seam (the
//! same `start$` / `stop$` control-mutation pair the JS bridge returns).

use std::rc::Rc;

use crate::core::edgy::reactive::{AtomId, Derived, Readable, Source};
use crate::core::edgy::value::Value;
use crate::core::view::View;
use rut_core::types::{TY_NIL, TY_OPAQUE, TY_STR, TY_U64};
use rut_vm::Opaque;

use super::{Intent, RutHandles};

/// The watch pair opaque (the `start$` / `stop$` control mutations).
pub struct RutWatch {
    start: crate::core::edgy::reactive::Mutation,
    stop: crate::core::edgy::reactive::Mutation,
}

/// Declare the C8 rows on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    vec![
        ("rs_derive", vec![TY_STR, TY_U64], TY_U64),
        ("rs_derive2", vec![TY_STR, TY_U64, TY_U64], TY_U64),
        ("el_text_bound_d", vec![TY_U64], TY_OPAQUE),
        ("rs_watch", vec![TY_U64, TY_STR, TY_U64], TY_OPAQUE),
        ("rs_watch_start", vec![TY_OPAQUE], TY_NIL),
        ("rs_watch_stop", vec![TY_OPAQUE], TY_NIL),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// Install the C8 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // rs_derive(name, dep) -> derived-id — the entry fn signature is
    // `entry fn name(v: f64) -> str`; the materialization reads the dep
    // through the tracked face (recording the dependency), then calls.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_derive", (&str, u64) -> u64, move |_vm: &mut rut_vm::interp::Vm, name: &str, dep: u64| {
        let name = name.to_string();
        let h2 = h.clone();
        let derived = h.store.bridge().build_derive(move |read, _boa| {
            let v = read
                .read(Readable::from(Source::<Value>::from_id(AtomId(dep as u32))), None)
                .as_num()
                .unwrap_or(0.0);
            match h2.face.call::<_, String>(&h2, &name, (v,)) {
                Ok(s) => Ok(Value::str(s.as_str())),
                // Reported by the face (error rail); the derived falls
                // back to Nil — the flush never aborts.
                Err(_) => Ok(Value::Nil),
            }
        });
        Ok(derived.id().0 as u64)
    });

    // rs_derive2(name, a, b) — two f64 deps, `entry fn name(a: f64, b: f64) -> str`.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_derive2", (&str, u64, u64) -> u64, move |_vm: &mut rut_vm::interp::Vm, name: &str, da: u64, db: u64| {
        let name = name.to_string();
        let h2 = h.clone();
        let derived = h.store.bridge().build_derive(move |read, _boa| {
            let va = read
                .read(Readable::from(Source::<Value>::from_id(AtomId(da as u32))), None)
                .as_num()
                .unwrap_or(0.0);
            let vb = read
                .read(Readable::from(Source::<Value>::from_id(AtomId(db as u32))), None)
                .as_num()
                .unwrap_or(0.0);
            match h2.face.call::<_, String>(&h2, &name, (va, vb)) {
                Ok(s) => Ok(Value::str(s.as_str())),
                Err(_) => Ok(Value::Nil),
            }
        });
        Ok(derived.id().0 as u64)
    });

    // el_text_bound_d(derived_id) — a Text bound to a DERIVED atom (the
    // str-carrying twin of `el_text_bound`).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_text_bound_d", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, derived: u64| {
        let _ = &h;
        let view = Rc::new(crate::builtin_plugins::text::TextView {
            text: Some(crate::core::view::Val::Reactive(Readable::Derived(
                Derived::<String>::from_id(AtomId(derived as u32)),
            ))),
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
        Ok(Opaque::alloc(vm, super::RutView(view as Rc<dyn View>))?.handle().clone())
    });

    // rs_watch(atom, cb, report) — the callback intent carries the report
    // atom (a) and the watched atom (b); the entry fn reads the fresh
    // value via the rs_get_* rows.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_watch", (u64, &str, u64) -> rut_vm::OpaqueRef, move |_vm: &mut rut_vm::interp::Vm, atom: u64, cb: &str, report: u64| {
        let name = cb.to_string();
        let h2 = h.clone();
        let dirty = h.dirty.clone();
        let mutation = h.store.bridge().build_mutate(move |_bridge, _args, _boa| {
            h2.pending_calls.borrow_mut().push(Intent::Click {
                name: name.clone(),
                a: report,
                b: atom,
                seq: 1.0,
            });
            dirty.set(true);
            Ok(Value::Nil)
        });
        let (start, stop) = h.store.bridge().register_watch(
            Readable::from(Source::<Value>::from_id(AtomId(atom as u32))).to_any(),
            mutation,
        );
        Ok(Opaque::alloc(_vm, RutWatch { start, stop })?.handle().clone())
    });

    // rs_watch_start / rs_watch_stop — the control mutations invoked
    // through the store rail (realm-free: Rust closures).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_watch_start", (Opaque<RutWatch>,) -> (), move |_vm: &mut rut_vm::interp::Vm, w: Opaque<RutWatch>| {
        let start = w.with(|w| w.start)?;
        h.store
            .bridge()
            .invoke_mutation(start, &[], None)
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_watch_start: {e}")))?;
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_watch_stop", (Opaque<RutWatch>,) -> (), move |_vm: &mut rut_vm::interp::Vm, w: Opaque<RutWatch>| {
        let stop = w.with(|w| w.stop)?;
        h.store
            .bridge()
            .invoke_mutation(stop, &[], None)
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_watch_stop: {e}")))?;
        Ok(())
    });
}
