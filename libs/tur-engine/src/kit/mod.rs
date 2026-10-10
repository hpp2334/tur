//! The kit — the authored element-construction surface.
//!
//! [`TUR_KIT_RUT`] is a rut SOURCE module: `rut/tur_kit/mod.rut` — the
//! walk's root module beside the kit manifest, the materialized
//! '\n'-join of the kit's authored files (one class per element over its
//! family's `tur_host` pkg rows: one method per prop, `.child` /
//! `.children` appending, `.build()` the only terminal). The embed here
//! and the manifest directory are the same bytes, and the `tur_host_decl`
//! pin test diffs the two. The kit lives OUTSIDE
//! `core/` (the layering law: core owns mechanism, never elements) and is
//! registered as a compile-session prelude by the standard bundle
//! assembly — [`crate::builtin_plugins::std::TurStdPlugin`] pushes it
//! through the pkg-extension seam's `preludes` channel, so `use
//! tur_kit::{ Column, … }` resolves in every module the standard plugin
//! set compiles. Core never names it.

/// The kit's source — `rut/tur_kit/mod.rut`, byte-identical to the
/// manifest directory the rut loader walks.
pub const TUR_KIT_RUT: &str = include_str!("../../../../rut/tur_kit/mod.rut");

/// The kit as a compile pkg (spec `tur_kit`) — what the standard bundle
/// assembly pushes into [`crate::core::rut_runtime::RutPkgCx::preludes`].
pub fn tur_kit_pkg() -> rut_driver::Pkg {
    rut_driver::Pkg {
        spec: "tur_kit".to_string(),
        namespace: Some("tur_kit".to_string()),
        body: rut_driver::PkgBody::Source {
            text: TUR_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
