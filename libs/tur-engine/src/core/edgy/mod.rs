//! The reactive substrate: native atoms (`Source` / `Derived` / `Mutation`),
//! the store KV, mutation invocations, and `watch` — Rust-native, no script
//! realm. The rut rail's `rs_*` rows and every plugin face are served by
//! [`reactive`] / [`mutation`].

pub mod mutation;
pub mod reactive;
pub mod value;
pub(crate) mod watch;

pub use value::{FromValue, Value};
