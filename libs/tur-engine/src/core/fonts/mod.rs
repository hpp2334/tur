use parley::fontique::GenericFamily;
use parley::FontContext;

const DEFAULT_FONT: &[u8] = include_bytes!("../../../fonts/Roboto-Regular.ttf");

pub trait FontLoader {
    fn create_font_context(&self) -> FontContext;

    fn register_font(&self, font_cx: &mut FontContext, name: &str, data: &[u8]);
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

    fn register_preset_fonts(&self, fcx: &mut FontContext) {
        let families = fcx.collection.register_fonts(DEFAULT_FONT.to_vec());
        let family_ids = families.into_iter().map(|(id, _)| id);
        fcx.collection
            .set_generic_families(GenericFamily::SansSerif, family_ids);
    }
}

impl FontLoader for PresetFontLoader {
    fn create_font_context(&self) -> FontContext {
        let mut fcx = FontContext::new();
        self.register_preset_fonts(&mut fcx);
        fcx
    }

    fn register_font(&self, _font_cx: &mut FontContext, _name: &str, _data: &[u8]) {}
}
