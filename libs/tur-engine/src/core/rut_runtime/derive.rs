//! C8 — derived atoms + `watch` on the rut rail: the decided law — a
//! **synchronous VM call during flush**, guarded.
//!
//! The str-typed derive rows (`rs_derive_cb` / `rs_derive2_cb`) are GONE —
//! the kit's `derive<T>` mints through `derive_seal`'s boxed lane (the
//! `__tur_cb_derive_box` entry + the kind probes), whose `ctx.get` reads
//! are the same tracked read face. The face applies every guard the plan
//! decided:
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
//! `rs_watch_mut(atom, sealed)` rides the sanctioned `register_watch` seam
//! (the same `start$` / `stop$` control-mutation pair the JS bridge
//! returned): the callback IS the kit-sealed `Mutation<nil>` composition
//! atom, delivered through the mutation queue (the kit's `__tur_cb_mnil`
//! lane) — no intent, no report param, no click-lane borrowing.

use std::rc::Rc;

use crate::core::edgy::reactive::{AtomId, Readable, Source};
use crate::core::edgy::value::Value;
use rut_vm::Opaque;

use super::RutHandles;
use super::mutation::mutation_of;

/// The watch pair opaque (the `start$` / `stop$` control mutations).
pub struct RutWatch {
    start: crate::core::edgy::reactive::Mutation,
    stop: crate::core::edgy::reactive::Mutation,
}

/// Install the C8 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // rs_watch_mut(atom, sealed) -> opaque — the boa-model watch: the
    // callback IS the kit-sealed `Mutation<nil>` composition atom (the
    // kit's `cb.seal(SEAL_NIL)` — the mutate_seal rail), registered
    // verbatim as the watcher callback. Delivery rides the mutation
    // queue (the flush pushes due callbacks; `invoke_mutation_by_id`
    // arms the watch-loop guard for it) and fires the kit's
    // `__tur_cb_mnil(sealed, marker)` entry — the nil-payload lane
    // `on_click` uses — constructing the `MutationCtx` over the bridge.
    // No intent push, no report param, no click-lane borrowing.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_watch_mut", (u64, u64) -> rut_vm::OpaqueRef, move |_vm: &mut rut_vm::interp::Vm, atom: u64, sealed: u64| {
        let (start, stop) = h.store.bridge().register_watch(
            Readable::from(Source::<Value>::from_id(AtomId(atom as u32))).to_any(),
            mutation_of(sealed),
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
