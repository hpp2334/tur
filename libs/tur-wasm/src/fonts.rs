use parley::FontContext;
use parley::fontique::GenericFamily;
use tur_engine::core::fonts::FontLoader;

const DEFAULT_FONT: &[u8] = include_bytes!("../fonts/Roboto-VF.ttf");
const MONO_FONT: &[u8] = include_bytes!("../fonts/RobotoMono-VF.ttf");

/// [`FontLoader`] for the wasm embedder: registers the two fonts bundled
/// into the wasm binary (a variable Roboto + Roboto Mono) and maps the
/// generic families to them.
///
/// Roboto is a **variable** font (`wght` axis 100–900), so a requested
/// `fontWeight` (100–1000) renders a true face on the web instead of
/// falling back to a single static weight. On native targets the
/// integration tests use [`tur_native::NativeFontLoader`] instead, which
/// discovers installed system fonts. The browser has no filesystem access,
/// so we ship a minimal, predictable face set here.
///
/// [`tur_native::NativeFontLoader`]: https://docs.rs/tur-native
pub struct WasmFontLoader;

impl Default for WasmFontLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmFontLoader {
    pub fn new() -> Self {
        Self
    }
}

impl FontLoader for WasmFontLoader {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        // register_fonts reads the family names from the TTF name tables, so
        // `font_family("Roboto")` / `font_family("Roboto Mono")` resolve by
        // NAME in the resolver's stack (the primary entry), not just via the
        // generic aliases below.
        let families = fcx
            .collection
            .register_fonts(DEFAULT_FONT.to_vec().into(), None);
        let roboto_ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
        // Roboto is the bundled sans face — and the engine default family's
        // target: `resolve_font_stack` always appends `sans-serif` as the
        // stack tail, so this mapping is the wasm default-font assignment.
        // (Serif maps to it too — the anti-blank alias; nothing resolves the
        // serif generic today, but a future serif face lands here.)
        fcx.collection
            .set_generic_families(GenericFamily::SansSerif, roboto_ids.iter().copied());
        fcx.collection
            .set_generic_families(GenericFamily::Serif, roboto_ids.iter().copied());

        // A real monospace face (Roboto Mono) for the code editor, which
        // spells `font_family("Roboto Mono")` — by name, Flutter-style.
        let mono_families = fcx
            .collection
            .register_fonts(MONO_FONT.to_vec().into(), None);
        let mono_ids: Vec<_> = mono_families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(GenericFamily::Monospace, mono_ids.iter().copied());
    }
}
