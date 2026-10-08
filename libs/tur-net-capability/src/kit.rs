//! The net capability's kit prelude — the authored face over this
//! plugin's `tur_host` pkg rows (`net_stream`). Lives in the crate that
//! owns the rows (the layering law applied to the kit): the plugin pushes
//! it as a compile-session prelude next to its rows.

/// The net kit's rut source.
pub const NET_KIT_RUT: &str = include_str!("../../../rut/tur_net_kit/kit.rut");

/// The kit as a compile pkg (spec `tur_net_kit`).
pub fn net_kit_pkg() -> rut_driver::Pkg {
    rut_driver::Pkg {
        spec: "tur_net_kit".to_string(),
        namespace: Some("tur_net_kit".to_string()),
        body: rut_driver::PkgBody::Source {
            text: NET_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
