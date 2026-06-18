use parley::fontique::GenericFamily;
use parley::FontContext;

const DEFAULT_FONT: &[u8] = include_bytes!("../../../fonts/Roboto-Regular.ttf");

pub trait FontLoader {
    fn load_preset_fonts(&self, fcx: &mut FontContext);

    fn register_font(&self, _fcx: &mut FontContext, _name: &str, _data: &[u8]) {}
}

pub struct FontManager {
    inner: FontContext,
    loader: Box<dyn FontLoader>,
}

impl FontManager {
    pub fn new(loader: Box<dyn FontLoader>) -> Self {
        let mut fcx = FontContext::new();
        loader.load_preset_fonts(&mut fcx);
        Self { inner: fcx, loader }
    }

    pub fn font_context(&mut self) -> &mut FontContext {
        &mut self.inner
    }

    pub fn register_font(&mut self, name: &str, data: &[u8]) {
        self.loader.register_font(&mut self.inner, name, data);
    }
}

pub struct PresetFontLoader;

impl Default for PresetFontLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetFontLoader {
    pub fn new() -> Self {
        Self
    }
}

impl FontLoader for PresetFontLoader {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        let families = fcx.collection.register_fonts(DEFAULT_FONT.to_vec().into(), None);
        let family_ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
        // Roboto is the only bundled face for now; map every generic family
        // to it so that `fontFamily: "monospace"` / `"serif"` (e.g. the code
        // editor) still resolve to renderable glyphs rather than blanks.
        // TODO: bundle a true monospace face and map Monospace to it.
        let roboto_ids = family_ids.iter().copied();
        fcx.collection
            .set_generic_families(GenericFamily::SansSerif, roboto_ids);
        let roboto_ids = family_ids.iter().copied();
        fcx.collection
            .set_generic_families(GenericFamily::Monospace, roboto_ids);
        let roboto_ids = family_ids.iter().copied();
        fcx.collection
            .set_generic_families(GenericFamily::Serif, roboto_ids);
    }
}
