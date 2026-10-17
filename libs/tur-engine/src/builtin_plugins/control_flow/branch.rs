//! The lazy branch factory — the zero-arg twin of Each's item builder.
//!
//! Condition's `then_build` / `else_build` and Switch's `case_build`
//! store a kit-sealed fn box (`fn() -> opaque`) instead of a pre-built
//! subtree: the branch is [`ViewFactory::create`]-invoked at ACTIVATION
//! through the guarded VM face (`__tur_cb_build0`), so it reads LIVE
//! state each time it mounts, and RE-invoked at every re-activation (the
//! boa callback semantic — the countdown edit modal reads the draft at
//! open time). The call is fuel-capped, depth-limited, no-mount-guarded,
//! and traps are reported through the error rail — the same flush-time
//! call law [`EachBuilder`](super::each::EachBuilder) formalizes for item
//! builders.

use std::rc::Rc;

use crate::core::rut_runtime::{RutHandles, VmFace, cb_entries, opaque_to_view};
use crate::core::view::{View, ViewFactory};

use rut_vm::OpaqueRef;

/// A branch factory over a sealed fn box: cb + everything a guarded face
/// call needs (the row captures `face` / `handles` off the boot handles).
#[derive(Clone)]
pub struct BranchBuilder {
    pub cb: OpaqueRef,
    pub face: Rc<VmFace>,
    pub handles: Rc<RutHandles>,
}

impl ViewFactory for BranchBuilder {
    fn create(&self) -> Option<Rc<dyn View>> {
        let handle: Result<OpaqueRef, _> =
            self.face
                .call(&self.handles, cb_entries::BUILD0, (self.cb.clone(),));
        // Face calls report their own traps (error rail); a failed branch
        // degrades to "not built".
        handle.ok().and_then(|h| opaque_to_view(&h))
    }
}
