use std::sync::Arc;

pub use parley::FontContext;

/// Built-in platform-symbol fallback slice ("Tur Symbols", a renamed subset
/// of DejaVu Sans — see `LICENSE-TurSymbols.txt` next to this file): ⌘ U+2318,
/// ←↑→↓ U+2190-2193, ⇥ U+21E5, ⇧ U+21E7, ⌥ U+2325, ⌫ U+232B, ⏎ U+23CE.
///
/// The bundled Roboto faces (wasm) map none of these — the playground status
/// bar's "⌘S to run" rendered a tofu box — and stripped-down native systems
/// can lack them too. The slice rides in the engine binary and is installed
/// by [`install_symbol_fallback`] as the LAST face on the generic families,
/// so it is consulted only when the primary faces lack coverage.
const SYMBOL_FALLBACK_FONT: &[u8] = include_bytes!("TurSymbols-Regular.ttf");

/// Registers [`SYMBOL_FALLBACK_FONT`] and appends its family to the generic
/// families (sans/serif/mono) after the loader's own faces.
///
/// Must run AFTER `FontLoader::load_preset_fonts` — the wasm loader maps its
/// generics with `set_generic_families` (a REPLACE), which would drop an
/// earlier registration. We append to the generic families rather than the
/// fontique fallback map because `set/append_fallbacks` on a key BYPASSES the
/// platform system fallback for that script (the embedder entry wins outright),
/// while a generic-family append merely adds a last-resort face to the style
/// stack — the per-cluster query still consults system fallbacks afterwards.
fn install_symbol_fallback(fcx: &mut FontContext) {
    use parley::fontique::GenericFamily;

    let families = fcx
        .collection
        .register_fonts(SYMBOL_FALLBACK_FONT.to_vec().into(), None);
    let ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
    for generic in [
        GenericFamily::SansSerif,
        GenericFamily::Serif,
        GenericFamily::Monospace,
    ] {
        fcx.collection
            .append_generic_families(generic, ids.iter().copied());
    }
}

/// The fresh-font-stack contract: preset fonts from the platform loader,
/// then the built-in symbol fallback (order matters — see
/// [`install_symbol_fallback`]). The ONE sequence a fresh [`FontContext`]
/// gets its fonts from; shared by `TurRuntime::build`, [`FontManager::new`]
/// and the coverage tests.
pub fn load_font_stack(fcx: &mut FontContext, loader: &dyn FontLoader) {
    loader.load_preset_fonts(fcx);
    install_symbol_fallback(fcx);
}

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
        load_font_stack(&mut fcx, loader.as_ref());
        Self::from_context(fcx, loader)
    }

    pub fn font_context(&mut self) -> &mut FontContext {
        &mut self.inner
    }

    pub fn register_font(&mut self, name: &str, data: &[u8]) {
        self.loader.register_font(&mut self.inner, name, data);
    }
}
