//! The playground compile service's kit prelude — the authored face over the
//! `pg_*` rows (`pg_files_put` / `pg_compile` / `pg_compile_diags` /
//! `pg_highlight` / `pg_apply_highlight`). Owned by the plugin that owns the
//! rows (the layering law applied to the kit):
//! [`TurRutPlaygroundPlugin`](crate::TurRutPlaygroundPlugin) pushes it as a
//! compile-session prelude beside the rows, so hosts without the playground
//! service never see (or compile) it.

/// The playground kit's rut source.
pub const PLAYGROUND_KIT_RUT: &str = include_str!("../../../rut/tur_playground_kit/mod.rut");

/// The kit as a compile pkg (spec `tur_playground_kit`).
pub fn playground_kit_pkg() -> rut_driver::Mounted {
    rut_driver::Mounted {
        spec: "tur_playground_kit".to_string(),
        manifest: rut_driver::Manifest::lib("tur_playground_kit", &["tur_host", "tur_kit"]),
        body: rut_driver::MountedBody::Source {
            text: PLAYGROUND_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
