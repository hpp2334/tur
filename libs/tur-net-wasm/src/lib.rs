//! Browser HTTP backend for tur, backed by [`reqwest_wasm`].
//!
//! Re-exports the full net surface from [`tur_net_capability`] so browser
//! embedders only need this one crate. The backend ([`WasmHttp`]) is
//! registered via `TurRuntimeBuilder::capability(Http::new(WasmHttp))`.

mod backend;

pub use backend::{WasmHttp, perform_request};
pub use tur_net_capability::{
    Http, HttpBackend, HttpBody, HttpOutcome, NoopHttp, RequestOpts, ResponseType, TurNetPlugin,
};
