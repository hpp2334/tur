//! The lazy-font capability: the host's byte provider for declared faces.
//!
//! The engine's font rail (`core::fonts::FontRail`) owns declarations and
//! registrations; THIS capability answers "where do the bytes come from" —
//! the host owns locations (wasm: a CDN catalog over `fetch`; android: APK
//! assets / reqwest; tests: stub bytes). The engine never sees a URL.
//!
//! Embedders register it via
//! [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder::capability)
//! with `Fonts::new(provider)`; `TurFontPlugin` feature-detects it at
//! register and skips the font rows when absent (the `TurNetPlugin`
//! pattern).

use std::sync::Arc;

use std::pin::Pin;

use futures::future::Future;
use crate::core::capability::Capability;
use crate::core::fonts::FontFaceMeta;
use crate::error::TurError;

/// The host's lazy byte source for declared faces. Methods return boxed
/// futures (the trait is held as `dyn`); backends decide whether the load
/// is actually async — a test stub returns `std::future::ready(...)`.
///
/// The future is NOT `Send` (the rows drive it through
/// `InstanceContext::spawn_local`, the clipboard rows' rail) — wasm fetch
/// futures are `!Send`.
pub trait FontDataProvider: Send + Sync + 'static {
    /// Fetch the bytes for a declared face. The meta carries the face
    /// identity (family + the `overridden_*` selectors) — the provider maps
    /// it to a location by its own policy.
    fn load(
        &self,
        meta: &FontFaceMeta,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, TurError>> + '_>>;
}

/// Capability newtype wrapping an `Arc<dyn FontDataProvider>`. Registered
/// via `TurRuntimeBuilder::capability` with `Fonts::new(provider)`; the
/// font rows look it up at call time via
/// `RutHandles::inst.capability().of::<Fonts>()`.
#[derive(Clone)]
pub struct Fonts(Arc<dyn FontDataProvider>);

impl Fonts {
    /// Wrap a provider in the capability newtype.
    pub fn new(provider: impl FontDataProvider + 'static) -> Self {
        Self(Arc::new(provider))
    }

    /// Wrap an already-shared provider (hosts that pre-configure the
    /// provider behind an `Arc` and keep mutating handles to it).
    pub fn shared(provider: Arc<dyn FontDataProvider>) -> Self {
        Self(provider)
    }

    /// Borrow the underlying provider handle.
    pub fn backend(&self) -> &Arc<dyn FontDataProvider> {
        &self.0
    }
}

impl Capability for Fonts {}
