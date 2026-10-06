//! M1 — the mutation rail: the `ctx_*` read/write/compose rows and the
//! mutation/derive sealers. The write-side twin of [`crate::core::rut_runtime::derive`]
//! (the guarded flush-time face machinery): a sealed mutation's Rust
//! closure invokes the kit-sealed fn box through the face with the
//! store-bridge handle + the queued payload, and the kit's dispatch entry
//! constructs `MutationCtx.over(h)` (+ the typed event) and calls the
//! user's fn. Reads through `ctx_get_*` are the TRACKED read face — a
//! derive materialization's `ctx.get` records its dependencies exactly
//! like the JS-era derives did.
//!
//! The sealers (rows): `mutate_seal(cb, tag)` / `derive_seal(cb, tag)`
//! mint the atom and bake the dispatch entry (selected by the tag) into
//! the Rust closure. The kit spells the tags as its own consts — one law,
//! two spellings (see [`seal_tags`]).

use std::rc::Rc;

use crate::core::edgy::mutation::{MutationHandle, ValueArgs};
use crate::core::edgy::reactive::Mutation;
use crate::core::edgy::reactive::AtomId;
use crate::core::edgy::value::Value;
use rut_vm::{Opaque, OpaqueRef};

use super::{RutHandles, mutation_entries, seal_tags};

/// The store-bridge marker — the `h: opaque` the ctx classes carry. The
/// ctx rows are per-instance (they close over the instance's store), so
/// the marker is the routing token the adapter's ctx methods hand back;
/// a wrong marker fails the row decode (a checked error, never UB).
pub struct CtxBridge;

/// Rebuild a mutation handle from a raw atom id — the rut rows' crossing
/// (the ids ARE the atoms). Engine-pub so the pkg extensions (the gesture
/// rows) can store sealed mutations.
pub fn mutation_of(atom: u64) -> Mutation {
    Mutation::from_id(AtomId(atom as u32))
}

/// The write-side law: a `ctx.set` targets a SOURCE. A derived atom (or a
/// mutation atom) rejects the write — a checked error the row turns into
/// a trap (the error rail reports it; the flush continues).
fn checked_writable(handles: &RutHandles, atom: u64) -> Result<(), String> {
    if handles.store.bridge().is_source(AtomId(atom as u32)) {
        Ok(())
    } else {
        Err(format!(
            "ctx.set: atom {atom} is not a source — derived atoms are read-only"
        ))
    }
}

/// Install the M1 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // ctx_bridge() -> opaque — mint a store-bridge marker (the ctx
    // classes' `h`; `TaskCtx.mint()` builds its task-scoped ctx over one).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_bridge", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let _ = &h;
        Ok(Opaque::alloc(vm, CtxBridge)?.handle().clone())
    });

    // ctx_get_f64(h, atom) -> f64 — the TRACKED read (inside a derive
    // materialization the read records its dependency).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_get_f64", (Opaque<CtxBridge>, u64) -> f64, move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64| {
        match h.store.read_only().read(super::readable_of::<f64>(atom)).as_num() {
            Some(n) => Ok(n),
            None => Ok(0.0),
        }
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_get_str", (Opaque<CtxBridge>, u64) -> String, move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64| {
        match h.store.read_only().read(super::readable_of::<String>(atom)).as_str() {
            Some(s) => Ok(s.to_string()),
            None => Ok(String::new()),
        }
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_get_bool", (Opaque<CtxBridge>, u64) -> bool, move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64| {
        match h.store.read_only().read(super::readable_of::<bool>(atom)).as_bool() {
            Some(b) => Ok(b),
            None => Ok(false),
        }
    });

    // ctx_set_f64 / ctx_set_str / ctx_set_bool — the write rail, guarded:
    // only source atoms accept writes (a derived target is a checked
    // error — the trap rides the error rail).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_set_f64", (Opaque<CtxBridge>, u64, f64) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, v: f64| {
        checked_writable(&h, atom)
            .and_then(|()| {
                h.store
                    .bridge()
                    .set_source(super::source_of::<Value>(atom), Value::Num(v))
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ctx_set_f64: {e}")))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_set_str", (Opaque<CtxBridge>, u64, &str) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, v: &str| {
        checked_writable(&h, atom)
            .and_then(|()| {
                h.store
                    .bridge()
                    .set_source(super::source_of::<Value>(atom), Value::str(v))
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ctx_set_str: {e}")))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_set_bool", (Opaque<CtxBridge>, u64, bool) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, v: bool| {
        checked_writable(&h, atom)
            .and_then(|()| {
                h.store
                    .bridge()
                    .set_source(super::source_of::<Value>(atom), Value::Bool(v))
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ctx_set_bool: {e}")))
    });

    // ctx_run_nil(h, atom) — compose by QUEUEING the invocation: a
    // mutation body runs inside a face call (the VM is mid-`call`, borrow
    // held), so a nested synchronous invocation would re-borrow the VM —
    // rut's no-reentrancy law. The queued invocation drains at the next
    // mutation pass (boa's `ctx.set(navigateToRoot)` shape, deferred).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_run_nil", (Opaque<CtxBridge>, u64) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64| {
        let m = MutationHandle::<()>::new(mutation_of(atom));
        h.mutation_queue.borrow_mut().push(m, ());
        h.dirty.set(true);
        Ok(())
    });
    // ctx_run_f64(h, atom, a) — typed-arg composition, same queue law: the
    // arg crosses as the invocation's payload; the target's return lands
    // in the flush's invocation result (composition reads effects through
    // sources — there is no synchronous return across the VM boundary).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_run_f64", (Opaque<CtxBridge>, u64, f64) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, a: f64| {
        let m = MutationHandle::<ValueArgs>::new(mutation_of(atom));
        h.mutation_queue.borrow_mut().push(m, ValueArgs(vec![Value::Num(a)]));
        h.dirty.set(true);
        Ok(())
    });

    // mutate_seal(cb, tag) -> atom — mint a mutation whose closure
    // face-calls the kit dispatch entry named by the tag with the sealed
    // fn box, the bridge marker, and the queued payload.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mutate_seal", (OpaqueRef, u64) -> u64, move |vm: &mut rut_vm::interp::Vm, cb: OpaqueRef, tag: u64| {
        let marker = Opaque::alloc(vm, CtxBridge)?.handle().clone();
        let h2 = h.clone();
        let mutation = h.store.bridge().build_mutate(move |_bridge, args| {
            match tag {
                seal_tags::MUT_PTR => {
                    // The pointer crossing: [local.x, local.y, global.x,
                    // global.y, button] (the gesture queue's payload).
                    let num = |i: usize| match args.get(i) {
                        Some(Value::Num(n)) => *n,
                        _ => 0.0,
                    };
                    let button = num(4) as u64;
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MPTR,
                        (cb.clone(), marker.clone(), num(0), num(1), num(2), num(3), button),
                    );
                }
                seal_tags::MUT_F64 => {
                    let a = match args.first() {
                        Some(Value::Num(n)) => *n,
                        _ => 0.0,
                    };
                    match h2.face.call::<_, f64>(
                        &h2,
                        mutation_entries::MF64,
                        (cb.clone(), marker.clone(), a),
                    ) {
                        Ok(v) => return Ok(Value::Num(v)),
                        // Reported by the face (error rail); the flush
                        // never aborts.
                        Err(_) => return Ok(Value::Nil),
                    }
                }
                _ => {
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MNIL,
                        (cb.clone(), marker.clone()),
                    );
                }
            }
            Ok(Value::Nil)
        });
        Ok(mutation.id().0 as u64)
    });

    // derive_seal(cb, tag) -> atom — mint a derived whose closure
    // face-calls the kit dispatch entry named by the tag. The user fn's
    // `ctx.get` reads flow through the ctx rows INSIDE this closure — the
    // tracker frame is up, so dependencies auto-record (the rs_derive_cb
    // machinery's first-class twin).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "derive_seal", (OpaqueRef, u64) -> u64, move |vm: &mut rut_vm::interp::Vm, cb: OpaqueRef, tag: u64| {
        let marker = Opaque::alloc(vm, CtxBridge)?.handle().clone();
        let h2 = h.clone();
        let derived = h.store.bridge().build_derive(move |_read| {
            let out = match tag {
                seal_tags::DRV_STR => h2
                    .face
                    .call::<_, String>(&h2, mutation_entries::DERIVE_STR, (cb.clone(), marker.clone()))
                    .map(|s| Value::str(s.as_str())),
                seal_tags::DRV_BOOL => h2
                    .face
                    .call::<_, bool>(&h2, mutation_entries::DERIVE_BOOL, (cb.clone(), marker.clone()))
                    .map(Value::Bool),
                _ => h2
                    .face
                    .call::<_, f64>(&h2, mutation_entries::DERIVE_F64, (cb.clone(), marker.clone()))
                    .map(Value::Num),
            };
            // Reported by the face on error (error rail); the derived
            // falls back to Nil — the flush never aborts.
            Ok(out.unwrap_or(Value::Nil))
        });
        Ok(derived.id().0 as u64)
    });
}
