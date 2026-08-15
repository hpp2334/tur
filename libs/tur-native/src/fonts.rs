use parley::FontContext;
use tur_engine::core::fonts::FontLoader;

/// [`FontLoader`] that populates the font context from the host operating
/// system's installed fonts.
///
/// Delegates to fontique's platform backend (CoreText / DirectWrite /
/// fontconfig), which both registers every installed font and maps the
/// generic families (`SansSerif`, `Serif`, `Monospace`, …) to the platform
/// defaults (e.g. Helvetica / Times / Courier on macOS). No bundled fonts
/// are required.
pub struct NativeFontLoader;

impl Default for NativeFontLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFontLoader {
    pub fn new() -> Self {
        Self
    }
}

impl FontLoader for NativeFontLoader {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        // fontique lazily enumerates the OS font set on the first call and
        // is idempotent thereafter (a subsequent call re-scans). Generic
        // family mappings come from the same backend, so nothing else to do.
        fcx.collection.load_system_fonts();
    }
}
