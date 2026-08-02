use std::sync::Arc;

pub use parley::FontContext;

/// Font loading + registration. Implementations must be `Send + Sync` so
/// the runtime can hold them behind `Arc<dyn FontLoader + Send + Sync>`
/// and share across worker threads (Phase 8 threaded mode).
pub trait FontLoader: Send + Sync {
    fn load_preset_fonts(&self, fcx: &mut FontContext);

    fn register_font(&self, _fcx: &mut FontContext, _name: &str, _data: &[u8]) {}
}

/// Per-instance font state. Wraps parley's [`FontContext`] (the font
/// database/layout scratch) plus a shared [`FontLoader`] for runtime font
/// registration.
///
/// The expensive part — building the `FontContext` (system-font discovery +
/// preset-font loading) — happens **once** on the [`TurRuntime`](crate::TurRuntime)
/// and is cheaply cloned per instance: `FontContext`/`fontique::Collection`/
/// `fontique::Collection`'s `System` are all `Arc`-backed, so a clone just bumps
/// refcounts. Each instance then owns an independent mutable `FontContext`
/// (its own fallback cache, its own registered fonts) while sharing the
/// scanned system-font data.
pub struct FontManager {
    inner: FontContext,
    loader: Arc<dyn FontLoader>,
}

impl FontManager {
    /// Wrap a (typically cloned) shared `FontContext` plus the shared loader.
    /// The caller is expected to have already loaded preset/system fonts into
    /// `fcx` once (on the runtime) — this does not re-load them.
    pub fn from_context(fcx: FontContext, loader: Arc<dyn FontLoader>) -> Self {
        Self { inner: fcx, loader }
    }

    /// Build a fresh `FontContext` (discovering system fonts) and load the
    /// loader's preset fonts into it. Used by standalone callers that don't
    /// share a runtime's pre-built context.
    pub fn new(loader: Arc<dyn FontLoader>) -> Self {
        let mut fcx = FontContext::new();
        loader.load_preset_fonts(&mut fcx);
        Self::from_context(fcx, loader)
    }

    pub fn font_context(&mut self) -> &mut FontContext {
        &mut self.inner
    }

    pub fn register_font(&mut self, name: &str, data: &[u8]) {
        self.loader.register_font(&mut self.inner, name, data);
    }
}
