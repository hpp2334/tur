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

use crate::core::edgy::mutation::MutationHandle;
pub use crate::core::edgy::mutation::ValueArgs;
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

    // ctx_get_value / ctx_set_value — the structured-value rail (the
    // list/map atoms' ctx face; the write side is guarded like the
    // scalars').
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_get_value", (Opaque<CtxBridge>, u64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64| {
        let value = h.store.read_value(AtomId(atom as u32)).unwrap_or(Value::Nil);
        Ok(Opaque::alloc(vm, super::RutValue(value))?.handle().clone())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_set_value", (Opaque<CtxBridge>, u64, Opaque<super::RutValue>) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, v: Opaque<super::RutValue>| {
        let value = v.with(|v| v.0.clone())?;
        checked_writable(&h, atom)
            .and_then(|()| {
                h.store
                    .bridge()
                    .set_source(super::source_of::<Value>(atom), value)
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ctx_set_value: {e}")))
    });

    // ctx_set_brush died with the brush wart: brush atoms are plain u64
    // sources now (the packed color rides the boxed write lane, the
    // `FromValue for Brush` decode reads the packed number).

    // ctx_get_opaque / ctx_set_opaque — the host-box rail (opaques are
    // just values): a Rust-held opaque (the animation controller slot)
    // rides a source atom as `Value::Opaque`, guarded like the other
    // writes.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_get_opaque", (Opaque<CtxBridge>, u64) -> rut_vm::OpaqueRef, move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64| {
        let value = h.store.read_value(AtomId(atom as u32)).unwrap_or(Value::Nil);
        let any = match value.as_opaque() {
            Some(a) => a,
            None => return Err(rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "ctx_get_opaque: the atom holds no opaque")),
        };
        match any.clone().downcast::<rut_vm::OpaqueRef>() {
            Ok(o) => Ok((*o).clone()),
            Err(_) => Err(rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "ctx_get_opaque: the opaque is not a host box")),
        }
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_set_opaque", (Opaque<CtxBridge>, u64, rut_vm::OpaqueRef) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, o: rut_vm::OpaqueRef| {
        checked_writable(&h, atom)
            .and_then(|()| {
                h.store
                    .bridge()
                    .set_source(super::source_of::<Value>(atom), Value::opaque(Rc::new(o) as Rc<dyn std::any::Any>))
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ctx_set_opaque: {e}")))
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

    // ---- the boxed rail's ctx face (the GENERIC lane) ---------------------
    //
    // ctx_get_box(h, atom, stamp) — the TRACKED read through the box: the
    // atom's native value re-seals as a rut erasure box so the kit's
    // generic `Source<T>.get()` recovers it with `unopaque<T>`. The stamp
    // picks the number lane's width (the KV's one collapse — see
    // [`super::STAMP_NUM_U64`]); every other kind stamps by its variant.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_get_box", (Opaque<CtxBridge>, u64, u64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, stamp: u64| {
        let value = h.store.read_only().read(super::readable_of::<Value>(atom));
        super::seal_box(vm, &value, stamp)
    });
    // ctx_set_box(h, atom, box) — the guarded generic write: the box
    // decodes into the same native variant the typed rows store (the
    // write-side law applies — only source atoms accept writes).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_set_box", (Opaque<CtxBridge>, u64, OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, v: OpaqueRef| {
        let value = super::boxed_to_value(vm, &v)?;
        checked_writable(&h, atom)
            .and_then(|()| {
                h.store
                    .bridge()
                    .set_source(super::source_of::<Value>(atom), value)
            })
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ctx_set_box: {e}")))
    });
    // ctx_run_box(h, atom, box) — the GENERIC composition row: the arg
    // crosses boxed (the kit's `mutate<A>` sealed the fixed-type wrapper
    // whose tag lands here); the payload decodes in the drain's MUT_BOX
    // branch and the wrapper recovers A with `unopaque<A>`.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ctx_run_box", (Opaque<CtxBridge>, u64, OpaqueRef) -> (), move |_vm: &mut rut_vm::interp::Vm, _bridge: Opaque<CtxBridge>, atom: u64, b: OpaqueRef| {
        let m = MutationHandle::<ValueArgs>::new(mutation_of(atom));
        h.mutation_queue
            .borrow_mut()
            .push(m, ValueArgs(vec![Value::opaque(Rc::new(b) as Rc<dyn std::any::Any>)]));
        h.dirty.set(true);
        Ok(())
    });

    // mutate_seal(cb, tag) -> atom — mint a mutation whose closure
    // face-calls the kit dispatch entry named by the tag with the sealed
    // fn box, the bridge marker, and the queued payload. Each tag is one
    // payload shape; the kit's entry constructs the ctx (+ the typed
    // event) and calls the user's fn (one entry per shape — the kit owns
    // the shapes, the engine owns the queue law).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mutate_seal", (OpaqueRef, u64) -> u64, move |vm: &mut rut_vm::interp::Vm, cb: OpaqueRef, tag: u64| {
        let marker = Opaque::alloc(vm, CtxBridge)?.handle().clone();
        let h2 = h.clone();
        let mutation = h.store.bridge().build_mutate(move |_bridge, args| {
            // The payload decoders (each tag's crossing shape).
            let num = |i: usize| match args.get(i) {
                Some(Value::Num(n)) => *n,
                _ => 0.0,
            };
            let text = |i: usize| match args.get(i) {
                Some(Value::Str(s)) => s.to_string(),
                _ => String::new(),
            };
            match tag {
                seal_tags::MUT_PTR => {
                    // [local.x, local.y, global.x, global.y, button].
                    let button = num(4) as u64;
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MPTR,
                        (cb.clone(), marker.clone(), num(0), num(1), num(2), num(3), button),
                    );
                }
                seal_tags::MUT_ENTER | seal_tags::MUT_EXIT => {
                    // [local.x, local.y, global.x, global.y] — the region
                    // crossing (the entry picks Enter vs Exit by its name).
                    let entry = if tag == seal_tags::MUT_ENTER {
                        mutation_entries::MENTER
                    } else {
                        mutation_entries::MEXIT
                    };
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        entry,
                        (cb.clone(), marker.clone(), num(0), num(1), num(2), num(3)),
                    );
                }
                seal_tags::MUT_KEY => {
                    // [key, code, modifiers].
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MKEY,
                        (cb.clone(), marker.clone(), text(0), text(1), num(2) as u64),
                    );
                }
                seal_tags::MUT_INPUT => {
                    // [value, enter].
                    let enter = match args.get(1) {
                        Some(Value::Bool(b)) => *b,
                        _ => false,
                    };
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MINPUT,
                        (cb.clone(), marker.clone(), text(0), enter),
                    );
                }
                seal_tags::MUT_BYTES => {
                    // [bytes].
                    let data = match args.first() {
                        Some(Value::Bytes(b)) => b.to_vec(),
                        _ => Vec::new(),
                    };
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MBYTES,
                        (cb.clone(), marker.clone(), data),
                    );
                }
                seal_tags::MUT_F64 => {
                    // [a] — the typed-arg mutation (ticks, `ctx.run_f64`'s
                    // target). No synchronous return: composition reads
                    // its effects through sources.
                    let a = num(0);
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MF64,
                        (cb.clone(), marker.clone(), a),
                    );
                }
                seal_tags::MUT_BOX => {
                    // [box] — the generic composition lane: the kit's
                    // `invoke` crossed the arg as the erasure box; the
                    // entry hands it to the sealed fixed-type wrapper,
                    // which recovers A with `unopaque<A>`.
                    let b = match args.first() {
                        Some(Value::Opaque(a)) => a.downcast_ref::<OpaqueRef>().cloned(),
                        _ => None,
                    };
                    let Some(b) = b else {
                        return Err("ctx.run: the boxed payload is missing".to_string());
                    };
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        mutation_entries::MBOX,
                        (cb.clone(), marker.clone(), b),
                    );
                }
                seal_tags::MUT_FOCUS | seal_tags::MUT_BLUR | seal_tags::MUT_MOUNT => {
                    // The payload-less events — the entry constructs the
                    // typed event (FocusEvent / BlurEvent / MountEvent).
                    let entry = match tag {
                        seal_tags::MUT_FOCUS => mutation_entries::MFOCUS,
                        seal_tags::MUT_BLUR => mutation_entries::MBLUR,
                        _ => mutation_entries::MMOUNT,
                    };
                    let _ = h2.face.call::<_, ()>(
                        &h2,
                        entry,
                        (cb.clone(), marker.clone()),
                    );
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
                seal_tags::DRV_BOX => {
                    // The generic lane: the kit's wrapper calls the user
                    // derive fn and returns the result boxed; the kit's
                    // probe entries recover the kind + the typed value
                    // (the closure has no VM, so the unbox runs rut-side).
                    h2.face
                        .call::<_, OpaqueRef>(
                            &h2,
                            mutation_entries::DERIVE_BOX,
                            (cb.clone(), marker.clone()),
                        )
                        .and_then(|boxed| {
                            h2.face
                                .call::<_, u64>(&h2, mutation_entries::DERIVE_KIND, (boxed.clone(),))
                                .and_then(|kind| match kind {
                                    mutation_entries::BOX_KIND_STR => h2
                                        .face
                                        .call::<_, String>(&h2, mutation_entries::UNBOX_STR, (boxed,))
                                        .map(|s| Value::str(s.as_str())),
                                    mutation_entries::BOX_KIND_BOOL => h2
                                        .face
                                        .call::<_, bool>(&h2, mutation_entries::UNBOX_BOOL, (boxed,))
                                        .map(Value::Bool),
                                    mutation_entries::BOX_KIND_U64 => h2
                                        .face
                                        .call::<_, u64>(&h2, mutation_entries::UNBOX_U64, (boxed,))
                                        .map(|n| Value::Num(n as f64)),
                                    mutation_entries::BOX_KIND_F64 => h2
                                        .face
                                        .call::<_, f64>(&h2, mutation_entries::UNBOX_F64, (boxed,))
                                        .map(Value::Num),
                                    _ => Ok(Value::Nil),
                                })
                        })
                }
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
