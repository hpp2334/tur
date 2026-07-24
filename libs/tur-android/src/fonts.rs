use parley::fontique::GenericFamily;
use parley::FontContext;
use tur_engine::core::fonts::FontLoader;

const DEFAULT_FONT: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
const MONO_FONT: &[u8] = include_bytes!("../fonts/RobotoMono-VF.ttf");

/// [`FontLoader`] for the Android embedder: registers the two fonts bundled
/// into the `.so` (Roboto + Roboto Mono) and maps the generic families to
/// them. Mirrors the wasm embedder's `WasmFontLoader` — Android has no
/// `fontique` system-font backend, so we ship a minimal, predictable face set.
pub struct AndroidFontLoader;

impl Default for AndroidFontLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AndroidFontLoader {
    pub fn new() -> Self {
        Self
    }
}

impl FontLoader for AndroidFontLoader {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        let families = fcx
            .collection
            .register_fonts(DEFAULT_FONT.to_vec().into(), None);
        let roboto_ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
        // Roboto is the bundled sans-serif face. Map SansSerif and Serif to it
        // so that `fontFamily: "serif"` (etc.) still resolves to renderable
        // glyphs rather than blanks.
        fcx.collection
            .set_generic_families(GenericFamily::SansSerif, roboto_ids.iter().copied());
        fcx.collection
            .set_generic_families(GenericFamily::Serif, roboto_ids.iter().copied());

        // A real monospace face (Roboto Mono) for the code editor, which sets
        // `fontFamily: "monospace"`. Mapped to its own family id so it renders
        // truly monospaced instead of falling back to Roboto.
        let mono_families = fcx
            .collection
            .register_fonts(MONO_FONT.to_vec().into(), None);
        let mono_ids: Vec<_> = mono_families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(GenericFamily::Monospace, mono_ids.iter().copied());
    }
}
