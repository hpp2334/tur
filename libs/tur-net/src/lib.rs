//! HTTP networking plugin for tur.
//!
//! Provides the [`Http`] capability trait, the `builtin:tur/net` bridge
//! (Promise-returning `request(opts)`), and a [`TurNetPlugin`] that
//! conditionally registers the module when an `Http` impl is provided.
//!
//! ## Architecture
//!
//! - The trait + types live here (backends implement `Http`).
//! - The bridge closure lives in [`bridge`] — it parses JS opts into
//!   [`RequestOpts`], spawns the future via the engine's `AsyncExecutor`,
//!   and settles the `JsPromise` via a completion closure on the next
//!   `flush`.
//! - Backends (e.g. `WasmHttp` in tur-wasm, `RecordingHttp` in tests)
//!   inject their impl via [`TurNetPlugin::builder`].

pub mod bridge;

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;

// ---------------------------------------------------------------------------
// Http capability trait + supporting types
// ---------------------------------------------------------------------------

/// Request body kind. Mirrors what JS can pass via `request({ body })`:
/// either a string or an `ArrayBuffer` (from `pickFile`).
#[derive(Debug, Clone)]
pub enum HttpBody {
    Text(String),
    Bytes(Vec<u8>),
}

/// Response body kind the caller wants back. `"text"` (default) fills
/// `bodyText`; `"bytes"` fills `bodyBytes` as an `ArrayBuffer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    Text,
    Bytes,
}

/// Request options, parsed from the JS `{ url, method?, headers?, body?,
/// responseType?, username?, password? }` object.
#[derive(Debug, Clone)]
pub struct RequestOpts {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<HttpBody>,
    pub response_type: ResponseType,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Outcome of an HTTP request — the success body or the error message.
/// The bridge builds a JS object from this in the completion closure.
#[derive(Debug, Clone)]
pub enum HttpOutcome {
    Ok {
        status: u16,
        status_text: String,
        headers: Vec<(String, String)>,
        body: HttpBody,
    },
    Err(String),
}

/// Async HTTP capability. Backends provide an impl (`WasmHttp` via
/// `reqwest_wasm` on wasm; `RecordingHttp` for tests) and inject it through
/// [`TurNetPlugin::builder`]. The bridge fn `request` in `builtin:tur/net`
/// consumes it.
pub trait Http: 'static {
    fn request(&self, opts: RequestOpts) -> Pin<Box<dyn Future<Output = HttpOutcome>>>;
}

/// No-op `Http` default. Always rejects with "no http backend" — JS cases
/// feature-detect via `typeof request === "function"` (see github-viewer),
/// which the plugin honors by *not* registering `builtin:tur/net` when no
/// `Http` impl was provided.
#[derive(Default)]
pub struct NoopHttp;
impl Http for NoopHttp {
    fn request(&self, _opts: RequestOpts) -> Pin<Box<dyn Future<Output = HttpOutcome>>> {
        Box::pin(std::future::ready(HttpOutcome::Err(
            "no http backend".to_string(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// tur-net plugin: registers `builtin:tur/net` (with the `request` bridge
/// fn) when an [`Http`] impl is provided via the builder.
///
/// If no backend is injected, the plugin is a no-op — `builtin:tur/net`
/// remains unregistered, and JS code that imports from it fails at module
/// load. Cases that may run in HTTP-less environments must guard accordingly
/// (or be marked playground-only, like github-viewer).
///
/// The bridge fn (`request`) is a ctx-bound `Ptr` that reads its
/// `Rc<dyn Http>` and `Rc<AsyncExecutor>` from `TurJsContext`'s capability
/// registry (populated here during `register`, and by the engine for the
/// executor). This avoids `unsafe NativeFunction::from_closure` — see
/// [`bridge`].
#[derive(Default)]
pub struct TurNetPlugin {
    http: Option<Rc<dyn Http>>,
}

impl TurNetPlugin {
    pub fn builder() -> TurNetPluginBuilder {
        TurNetPluginBuilder::new()
    }
}

impl Plugin for TurNetPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        if let Some(http) = self.http.clone() {
            // Expose the Http backend to ctx-bound bridge fns (tur_net_request).
            // The executor capability is already inserted by the engine.
            ctx.js_ctx().insert_capability::<Rc<dyn Http>>(http);

            // Register `builtin:tur/net` with `request` as a ctx-bound fn.
            ctx.register_module("builtin:tur/net", bridge::fns(), vec![], vec![]);
        }
        Ok(())
    }
}

pub struct TurNetPluginBuilder {
    http: Option<Rc<dyn Http>>,
}

impl Default for TurNetPluginBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TurNetPluginBuilder {
    pub fn new() -> Self {
        Self { http: None }
    }

    /// Inject an HTTP backend. When set, the plugin registers
    /// `builtin:tur/net` (with the `request` bridge fn) during `register`.
    pub fn http<P: Http + 'static>(mut self, platform: P) -> Self {
        self.http = Some(Rc::new(platform));
        self
    }

    pub fn build(self) -> TurNetPlugin {
        TurNetPlugin { http: self.http }
    }
}
