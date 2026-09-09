//! Reactive-edge primitives exposed to JS:
//! - `source` / `derive` / `mutate` / `get` / `set` / `view` bridge fns.
//!
//! The Rust reactive substrate (`Store`, `Source`, `Derived`, `MutationHandle`,
//! mutation queue) lives in the child modules [`reactive`] / [`mutation`].

pub mod bridge;
pub mod mutation;
pub mod reactive;
pub(crate) mod watch;

use crate::core::js_runtime::helpers::FnEntry;

/// Aggregate bridge fns for the edgy (reactive-edge) domain.
pub fn fns() -> Vec<FnEntry> {
    let mut v: Vec<FnEntry> = Vec::new();
    v.extend(bridge::fns());
    v
}
