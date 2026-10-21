//! The filepicker capability's kit prelude — the authored face over this
//! plugin's `tur_host` pkg rows (`pick_file` / `save_file`). Lives in the
//! crate that owns the rows (the layering law applied to the kit): the
//! plugin pushes it as a compile-session prelude next to its rows.

/// The file kit's rut source.
pub const FILE_KIT_RUT: &str = include_str!("../../../rut/tur_file_kit/mod.rut");

/// The kit as a compile pkg (spec `tur_file_kit`).
pub fn file_kit_pkg() -> rut_driver::Mounted {
    rut_driver::Mounted {
        spec: "tur_file_kit".to_string(),
        namespace: Some("tur_file_kit".to_string()),
        manifest: rut_driver::Manifest::lib("tur_file_kit", &["tur_host"]),
        body: rut_driver::MountedBody::Source {
            text: FILE_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
