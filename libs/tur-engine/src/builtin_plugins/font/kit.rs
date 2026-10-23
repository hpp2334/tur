//! The font capability's kit prelude — the authored face over the lazy-font
//! rows (`font_declare` / `font_declare_face` / `font_load_data`). Owned by
//! the plugin that owns the rows (the layering law applied to the kit):
//! [`TurFontPlugin`](super::TurFontPlugin) pushes it as a compile-session
//! prelude next to the rows.

/// The font kit's rut source.
pub const FONT_KIT_RUT: &str = include_str!("../../../../../rut/tur_font_kit/mod.rut");

/// The kit as a compile pkg (spec `tur_font_kit`).
pub fn font_kit_pkg() -> rut_driver::Mounted {
    rut_driver::Mounted {
        spec: "tur_font_kit".to_string(),
        namespace: Some("tur_font_kit".to_string()),
        manifest: rut_driver::Manifest::lib("tur_font_kit", &["tur_host"]),
        body: rut_driver::MountedBody::Source {
            text: FONT_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
