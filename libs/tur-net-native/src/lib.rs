//! Native HTTP backend for tur, backed by the native [`reqwest`] crate.
//!
//! Re-exports the full net surface from [`tur_net_capability`] so native
//! embedders only need this one crate. The backend ([`NativeHttp`]) is
//! registered via
//! `TurEngineBuilder::capability(Http::new(NativeHttp::default()))`.
//!
//! On wasm this crate compiles as a near-empty stub (the `reqwest` dep is
//! target-gated to `cfg(not(target_family = "wasm"))`). Embedders targeting
//! wasm should depend on `tur-net-wasm` instead.

pub use tur_net_capability::{
    Http, HttpBackend, HttpBody, HttpOutcome, NoopHttp, RequestOpts, ResponseType, TurNetPlugin,
};

#[cfg(not(target_family = "wasm"))]
mod backend;

#[cfg(not(target_family = "wasm"))]
pub use backend::NativeHttp;
