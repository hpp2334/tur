//! The lazy-container item-builder face: a rut entry-builder (the rut
//! rail's guarded flush-time VM call).
//!
//! Item specs materialize by calling the kit-sealed fn box through the
//! [`VmFace`] via the kit's `__tur_cb_build1` dispatch entry — fuel-capped,
//! depth-limited, no-mount-guarded, traps reported (never aborting the
//! flush).

use std::rc::Rc;

use crate::core::rut_runtime::{RutHandles, VmFace};
use crate::core::rut_runtime::opaque_to_view;
use crate::core::view::View;
use rut_vm::OpaqueRef;

/// The item builder shared by `LazyList` / `LazyGrid` (and the `Table`
/// row/header builders): everything a face call needs, captured by the
/// `rs_lazy_list` / `rs_lazy_grid` / table rows.
#[derive(Clone)]
pub struct RutEntryBuilder {
    pub cb: OpaqueRef,
    pub face: Rc<VmFace>,
    pub handles: Rc<RutHandles>,
}

impl RutEntryBuilder {
    /// Resolve the item spec for `index`. A failed face call degrades to
    /// "not built" — face calls report their own traps (error rail).
    pub fn build(&self, index: u64) -> Option<Rc<dyn View>> {
        let handle: Result<OpaqueRef, _> =
            self.face.call(&self.handles, "__tur_cb_build1", (self.cb.clone(), index));
        match handle {
            Ok(h) => opaque_to_view(&h),
            Err(_) => None,
        }
    }
}
