//! HTTP networking capability for tur.
//!
//! Provides:
//!
//! - The [`HttpBackend`] capability trait + supporting types
//!   ([`RequestOpts`], [`HttpOutcome`], [`HttpBody`], [`ResponseType`]).
//! - The [`Http`] capability newtype wrapping `Rc<dyn HttpBackend>`,
//!   registered via
//!   [`tur_engine::TurEngineBuilder::capability`](`Http::new(backend)`).
//! - The [`NoopHttp`] default.
//! - The [`TurNetPlugin`] (unit struct) that conditionally registers the
//!   `tur:net` module (with the `request` bridge fn) when an [`Http`]
//!   capability is present.
//!
//! ## Architecture
//!
//! - Backends (`WasmHttp` in `tur-net-wasm`, `NativeHttp` in
//!   `tur-net-native`, `RecordingHttp` in `tur-integration-tests`)
//!   implement [`HttpBackend`] and are registered via
//!   `.capability(Http::new(backend))`.
//! - The bridge closure (in [`bridge`]) parses JS opts into [`RequestOpts`],
//!   spawns the future via the engine's `AsyncExecutor`, and settles the
//!   `JsPromise` via a completion closure on the next `flush`.
//! - [`TurNetPlugin`] does NOT declare a `requires` for [`Http`] — HTTP is
//!   an optional capability. If absent, the plugin simply skips registering
//!   `tur:net`, and JS code feature-detects via
//!   `typeof request === "function"`.

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
/// either a string or an `ArrayBuffer` (e.g. from `filePicker.pick()`).
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

/// Async HTTP backend. Backends provide an impl (`WasmHttp` via
/// `reqwest_wasm` on wasm; `NativeHttp` via native `reqwest`; `RecordingHttp`
/// for tests) and register it via
/// `TurEngineBuilder::capability(Http::new(backend))`. The bridge fn
/// `request` in `tur:net` consumes it.
pub trait HttpBackend: 'static {
    fn request(&self, opts: RequestOpts) -> Pin<Box<dyn Future<Output = HttpOutcome>>>;
}

/// No-op `HttpBackend` default. Always rejects with "no http backend" — JS
/// cases feature-detect via `typeof request === "function"` (see
/// github-viewer), which the plugin honors by *not* registering
/// `tur:net` when no `Http` capability is provided.
#[derive(Default)]
pub struct NoopHttp;
impl HttpBackend for NoopHttp {
    fn request(&self, _opts: RequestOpts) -> Pin<Box<dyn Future<Output = HttpOutcome>>> {
        Box::pin(std::future::ready(HttpOutcome::Err(
            "no http backend".to_string(),
        )))
    }
}

/// Capability newtype wrapping an `Rc<dyn HttpBackend>`. Registered via
/// [`tur_engine::TurEngineBuilder::capability`] with `Http::new(backend)`;
/// the bridge fn `request` in `tur:net` looks it up at call time.
#[derive(Clone)]
pub struct Http(Rc<dyn HttpBackend>);

impl Http {
    /// Wrap a backend in the capability newtype.
    pub fn new(backend: impl HttpBackend + 'static) -> Self {
        Self(Rc::new(backend))
    }

    /// Borrow the underlying backend handle.
    pub fn backend(&self) -> &Rc<dyn HttpBackend> {
        &self.0
    }
}

impl tur_engine::core::capability::Capability for Http {}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// tur-net plugin: registers `tur:net` (with the `request` bridge
/// fn) when an [`Http`] capability is registered.
///
/// If no backend is injected, the plugin is a no-op — `tur:net`
/// remains unregistered, and JS code that imports from it fails at module
/// load. Cases that may run in HTTP-less environments must guard accordingly
/// (or be marked playground-only, like github-viewer).
///
/// The bridge fn (`request`) is a ctx-bound `Ptr` that reads its [`Http`]
/// capability from `TurJsContext`'s capability registry at call time. This
/// avoids `unsafe NativeFunction::from_closure` — see [`bridge`].
pub struct TurNetPlugin;

impl Default for TurNetPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurNetPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        // Optional capability: if no Http backend is registered, skip
        // registering `tur:net`. JS code feature-detects.
        if !ctx.capability().contains::<Http>() {
            tracing::info!("TurNetPlugin: no Http capability registered; skipping tur:net");
            return Ok(());
        }
        ctx.register_module("tur:net", bridge::fns(), vec![], vec![]);
        Ok(())
    }
}
