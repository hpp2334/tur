use parley::fontique::GenericFamily;
use parley::FontContext;
use tur_engine::core::fonts::FontLoader;

const DEFAULT_FONT: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
const MONO_FONT: &[u8] = include_bytes!("../fonts/RobotoMono-VF.ttf");

/// [`FontLoader`] for the wasm embedder: registers the two fonts bundled
/// into the wasm binary (Roboto + Roboto Mono) and maps the generic
/// families to them.
///
/// On native targets the integration tests use [`tur_native::NativeFontLoader`]
/// instead, which discovers installed system fonts. The browser has no
/// filesystem access, so we ship a minimal, predictable face set here.
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
