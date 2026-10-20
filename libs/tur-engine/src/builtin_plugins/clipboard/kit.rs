//! The clipboard capability's kit prelude — the authored face over the
//! clipboard rows (`clipboard_read` / `clipboard_write`). Owned by the
//! plugin that owns the rows (the layering law applied to the kit):
//! [`TurClipboardPlugin`](super::TurClipboardPlugin) pushes it as a
//! compile-session prelude next to the rows.

/// The clipboard kit's rut source.
pub const CLIPBOARD_KIT_RUT: &str = include_str!("../../../../../rut/tur_clipboard_kit/mod.rut");

/// The kit as a compile pkg (spec `tur_clipboard_kit`).
pub fn clipboard_kit_pkg() -> rut_driver::Mounted {
    rut_driver::Mounted {
        spec: "tur_clipboard_kit".to_string(),
        namespace: Some("tur_clipboard_kit".to_string()),
        manifest: rut_driver::Manifest::lib("tur_clipboard_kit", &["tur_host"]),
        body: rut_driver::MountedBody::Source {
            text: CLIPBOARD_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
