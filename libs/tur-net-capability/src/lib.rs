//! HTTP networking capability for tur.
//!
//! Provides:
//!
//! - The [`HttpBackend`] capability trait + supporting types
//!   ([`RequestOpts`], [`HttpOutcome`], [`HttpBody`], [`ResponseType`]).
//! - The [`Http`] capability newtype wrapping `Rc<dyn HttpBackend>`,
//!   registered via
//!   [`tur_engine::TurRuntimeBuilder::capability`](`Http::new(backend)`).
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

use futures::StreamExt;
use futures::stream::LocalBoxStream;
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::error::TurError;

// ---------------------------------------------------------------------------
// Http capability trait + supporting types
// ---------------------------------------------------------------------------

/// Shorthand for the boxed future returned by [`HttpBackend::request`].
pub type HttpFuture = Pin<Box<dyn Future<Output = HttpOutcome>>>;

/// Shorthand for the boxed future returned by [`HttpBackend::request_stream`].
pub type HttpStreamFuture = Pin<Box<dyn Future<Output = Result<HttpStreamResponse, String>>>>;

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

/// A streaming HTTP response. The body is a `BoxStream` yielding byte chunks.
/// Used by `HttpBackend::request_stream`.
pub struct HttpStreamResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: LocalBoxStream<'static, Result<Vec<u8>, String>>,
}

/// Async HTTP backend. Backends provide an impl (`WasmHttp` via
/// `reqwest_wasm` on wasm; `NativeHttp` via native `reqwest`; `RecordingHttp`
/// for tests) and register it via
/// `TurRuntimeBuilder::capability(Http::new(backend))`. The bridge fn
/// `request` in `tur:net` consumes it.
pub trait HttpBackend: Send + Sync + 'static {
    fn request(&self, opts: RequestOpts) -> HttpFuture;

    /// Streaming variant: returns the response headers immediately, then the
    /// body as a stream of byte chunks. Default impl delegates to `request()`
    /// and wraps the body as a single-chunk stream.
    fn request_stream(&self, opts: RequestOpts) -> HttpStreamFuture {
        let fut = self.request(opts);
        Box::pin(async move {
            let outcome = fut.await;
            match outcome {
                HttpOutcome::Ok {
                    status,
                    status_text,
                    headers,
                    body,
                } => {
                    let chunk = match body {
                        HttpBody::Text(t) => t.into_bytes(),
                        HttpBody::Bytes(b) => b,
                    };
                    let body_stream = futures::stream::once(async move { Ok(chunk) }).boxed_local();
                    Ok(HttpStreamResponse {
                        status,
                        status_text,
                        headers,
                        body: body_stream,
                    })
                }
                HttpOutcome::Err(e) => Err(e),
            }
        })
    }
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
/// [`tur_engine::TurRuntimeBuilder::capability`] with `Http::new(backend)`;
/// the bridge fn `request` in `tur:net` looks it up at call time.
#[derive(Clone)]
pub struct Http(std::sync::Arc<dyn HttpBackend + Send + Sync>);

impl Http {
    /// Wrap a backend in the capability newtype.
    pub fn new(backend: impl HttpBackend + 'static) -> Self {
        Self(std::sync::Arc::new(backend))
    }

    /// Borrow the underlying backend handle.
    pub fn backend(&self) -> &std::sync::Arc<dyn HttpBackend + Send + Sync> {
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
/// capability from `TurInstanceContext`'s capability registry at call time. This
/// avoids `unsafe NativeFunction::from_closure` — see [`bridge`].
pub struct TurNetPlugin;

impl Default for TurNetPlugin {
    fn default() -> Self {
        Self
    }
}

impl Plugin for TurNetPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext<'_>) -> Result<(), TurError> {
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
