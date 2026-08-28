//! Native HTTP backend for tur, backed by the native [`reqwest`] crate running
//! on a **user-provided** tokio runtime.
//!
//! Re-exports the full net surface from [`tur_net_capability`] so native
//! embedders only need this one crate. The backend ([`NativeHttp`]) is
//! registered via
//! `TurRuntimeBuilder::capability(Http::new(NativeHttp::new(handle)))`, where
//! `handle: tokio::runtime::Handle` comes from a runtime the embedder owns.
//!
//! The engine itself (`tur-engine`) is tokio-free. Only this
//! crate depends on tokio, and it never builds or enters a runtime of its own
//! — the embedder does that and passes the handle in. See
//! [`backend`][crate::backend] for the bridge details.
//!
//! On wasm this crate compiles as a near-empty stub (the `reqwest` + `tokio`
//! deps are target-gated to `cfg(not(target_family = "wasm"))`). Embedders
//! targeting wasm should depend on `tur-net-wasm` instead.

pub use tur_net_capability::{
    Http, HttpBackend, HttpBody, HttpOutcome, NoopHttp, RequestOpts, TurNetPlugin,
};

#[cfg(not(target_family = "wasm"))]
mod backend;

#[cfg(not(target_family = "wasm"))]
pub use backend::NativeHttp;
