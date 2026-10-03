//! The animation family's kit prelude — the authored wrappers over this
//! plugin's `tur` host-pkg rows (`Opacity` / `Transform`). Lives in the
//! crate that owns the rows (the layering law applied to the kit): the
//! plugin pushes it as a compile-session prelude next to its rows, so a
//! plugin set without animation never sees (or compiles) these classes.

/// The animation kit's rut source.
pub const TUR_ANIM_KIT_RUT: &str = include_str!("kit.rut");

/// The kit as a compile-session module (spec `tur_anim_kit`).
pub fn tur_anim_kit_module() -> rut_driver::Module {
    rut_driver::Module {
        spec: "tur_anim_kit".to_string(),
        namespace: Some("tur_anim_kit".to_string()),
        body: rut_driver::ModuleBody::Source {
            text: TUR_ANIM_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
