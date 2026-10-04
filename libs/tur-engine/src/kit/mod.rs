//! The kit — the authored element-construction surface.
//!
//! [`TUR_KIT_RUT`] is a rut SOURCE module (one class per element over its
//! family's `tur` host-pkg rows: one method per prop, `.child` /
//! `.children` appending, `.build()` the only terminal). It lives OUTSIDE
//! `core/` (the layering law: core owns mechanism, never elements) and is
//! registered as a compile-session prelude by the standard bundle
//! assembly — [`crate::builtin_plugins::std::TurStdPlugin`] pushes it
//! through the pkg-extension seam's `preludes` channel, so `use
//! tur_kit::{ Column, … }` resolves in every module the standard plugin
//! set compiles. Core never names it.

/// The kit's rut source (the authored builder surface).
pub const TUR_KIT_RUT: &str = include_str!("tur_kit.rut");

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
