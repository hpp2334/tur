//! The lazy-container item-builder face: a `JsFunction` (the JS rail) or a
//! rut entry-builder (the rut rail's guarded flush-time VM call).
//!
//! The rut arm is the C2 slice's foundation for the C8 decided law: item
//! specs materialize by calling an `entry fn(index) -> opaque` through the
//! [`VmFace`] — fuel-capped, depth-limited, no-mount-guarded, traps
//! reported (never aborting the flush).

use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::builtins::JsFunction;

use crate::core::rut_runtime::{RutHandles, VmFace};
use crate::core::view::View;
use rut_vm::OpaqueRef;

use crate::core::rut_runtime::opaque_to_view;

/// The item builder face shared by `LazyList` / `LazyGrid`.
#[derive(Clone)]
pub enum ItemBuilder {
    /// The JS rail: `(index) => Element`.
    Js(JsFunction),
    /// The rut rail: `entry fn name(index: u64) -> opaque` through the
    /// guarded VM face.
    Rut(RutEntryBuilder),
}

/// The rut half of [`ItemBuilder`] — everything a face call needs, captured
/// by the `rs_lazy_list` / `rs_lazy_grid` rows.
#[derive(Clone)]
pub struct RutEntryBuilder {
    pub name: String,
    pub face: Rc<VmFace>,
    pub handles: Rc<RutHandles>,
}

impl ItemBuilder {
    /// Resolve the item spec for `index`. `realm` is the instance's realm
    /// when one exists (`None` on a realm-free instance — the JS arm
    /// degrades, the rut arm never needed it). `warned_builder_error` is
    /// the one-error-per-element flag (the JS arm owns it).
    pub fn build(
        &self,
        index: u64,
        realm: Option<&mut Context>,
        warned_builder_error: &mut bool,
    ) -> Option<Rc<dyn View>> {
        match self {
            ItemBuilder::Js(builder) => {
                let boa = realm?;
                let result = builder.call(
                    &boa_engine::JsValue::undefined(),
                    &[boa_engine::JsValue::from(index as f64)],
                    boa,
                );
                match result {
                    Ok(result) => crate::core::view::extract_view(&result),
                    Err(err) => {
                        if !*warned_builder_error {
                            *warned_builder_error = true;
                            tracing::error!(
                                "item builder threw for index {index} — item not \
                                 built (further failures silenced): {err}"
                            );
                        }
                        None
                    }
                }
            }
            ItemBuilder::Rut(entry) => {
                let _ = realm;
                let handle: Result<OpaqueRef, _> =
                    entry.face.call(&entry.handles, &entry.name, (index,));
                match handle {
                    Ok(h) => opaque_to_view(&h),
                    // Face calls report their own traps (error rail); a
                    // failed item degrades to "not built" like a throwing
                    // JS builder.
                    Err(_) => None,
                }
            }
        }
    }
}
