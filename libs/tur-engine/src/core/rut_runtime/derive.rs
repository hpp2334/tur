//! C8 — derived atoms + `watch` on the rut rail: the decided law — a
//! **synchronous VM call during flush**, guarded.
//!
//! `rs_derive_cb(cb, dep)` mints a derived whose materialization reads its
//! dep through the tracked read face (auto-dependency tracking works
//! exactly as for JS derives) and then fires the kit-sealed format fn box
//! through the [`VmFace`] via the kit's `__tur_cb_str1` dispatch entry.
//! The face applies every guard the plan decided:
//!
//! - **fuel-capped** — the call runs on the VM's budget with bounded
//!   retry grants, then one drain grant that always returns the machine
//!   to idle (never parked mid-flush);
//! - **no-mount** — `face_busy` is raised for the call's duration; a
//!   derive that tries `tur_host::mount` traps (checked by the mount row);
//! - **depth-limited** — nested face calls cap at `VM_FACE_MAX_DEPTH`;
//! - **traps never abort the flush** — reported through the
//!   runtime-error rail, the derived falls back to `Value::Nil`.
//!
//! `rs_watch(atom, cb)` rides the sanctioned `register_watch` seam (the
//! same `start$` / `stop$` control-mutation pair the JS bridge returns).

use std::rc::Rc;

use crate::core::edgy::reactive::{AtomId, Readable, Source};
use crate::core::edgy::value::Value;
use rut_vm::{Opaque, OpaqueRef};

use super::{Intent, RutHandles, cb_entries};

/// The watch pair opaque (the `start$` / `stop$` control mutations).
pub struct RutWatch {
    start: crate::core::edgy::reactive::Mutation,
    stop: crate::core::edgy::reactive::Mutation,
}

/// Install the C8 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // rs_derive_cb(cb, dep) -> derived-id — the format fn is a fn value
    // with the `fn(v: f64) -> str` shape, kit-sealed; the materialization
    // reads the dep through the tracked face (recording the dependency),
    // then fires `__tur_cb_str1`.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_derive_cb", (OpaqueRef, u64) -> u64, move |_vm: &mut rut_vm::interp::Vm, cb: OpaqueRef, dep: u64| {
        let h2 = h.clone();
        let derived = h.store.bridge().build_derive(move |read| {
            let v = read
                .read(Readable::from(Source::<Value>::from_id(AtomId(dep as u32))))
                .as_num()
                .unwrap_or(0.0);
            match h2.face.call::<_, String>(&h2, "__tur_cb_str1", (cb.clone(), v)) {
                Ok(s) => Ok(Value::str(s.as_str())),
                // Reported by the face (error rail); the derived falls
                // back to Nil — the flush never aborts.
                Err(_) => Ok(Value::Nil),
            }
        });
        Ok(derived.id().0 as u64)
    });

    // rs_derive2_cb(cb, a, b) — two f64 deps, `fn(a: f64, b: f64) -> str`.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_derive2_cb", (OpaqueRef, u64, u64) -> u64, move |_vm: &mut rut_vm::interp::Vm, cb: OpaqueRef, da: u64, db: u64| {
        let h2 = h.clone();
        let derived = h.store.bridge().build_derive(move |read| {
            let va = read
                .read(Readable::from(Source::<Value>::from_id(AtomId(da as u32))))
                .as_num()
                .unwrap_or(0.0);
            let vb = read
                .read(Readable::from(Source::<Value>::from_id(AtomId(db as u32))))
                .as_num()
                .unwrap_or(0.0);
            match h2.face.call::<_, String>(&h2, "__tur_cb_str2", (cb.clone(), va, vb)) {
                Ok(s) => Ok(Value::str(s.as_str())),
                Err(_) => Ok(Value::Nil),
            }
        });
        Ok(derived.id().0 as u64)
    });

    // rs_watch_cb(atom, cb, report) — the callback intent carries the
    // kit-sealed fn box, the report atom (a) and the watched atom (b);
    // the callback reads the fresh value via the rs_get_* rows.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_watch_cb", (u64, OpaqueRef, u64) -> rut_vm::OpaqueRef, move |_vm: &mut rut_vm::interp::Vm, atom: u64, cb: OpaqueRef, report: u64| {
        let h2 = h.clone();
        let dirty = h.dirty.clone();
        let mutation = h.store.bridge().build_mutate(move |_bridge, _args| {
            h2.pending_calls.borrow_mut().push(Intent::Click {
                entry: cb_entries::CLICK,
                cb: cb.clone(),
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
            .invoke_mutation(start, &[])
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_watch_start: {e}")))?;
        Ok(())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_watch_stop", (Opaque<RutWatch>,) -> (), move |_vm: &mut rut_vm::interp::Vm, w: Opaque<RutWatch>| {
        let stop = w.with(|w| w.stop)?;
        h.store
            .bridge()
            .invoke_mutation(stop, &[])
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_watch_stop: {e}")))?;
        Ok(())
    });
}
