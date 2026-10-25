//! std_json — the std json offering: rut's std serde package (`json`)
//! and its dep closure (`strbuild` / `pouch` / `nmapset`, plus the
//! `strbuild_host` / `nmap_host` host pkgs whose bodies ship in
//! `rut-std`) mounted into the compile session's OFFERED set, so
//! modules spell `use json::{ decodeJson };` against the engine.
//!
//! Everything here is VENDORED from the pinned rut checkout (the
//! std-v14 tag rev in the workspace Cargo.toml) — provenance lives in
//! each file's header comment; re-vendor on rut pin bumps if upstream's
//! files change. The sources are mounted via `Mounted::source` (the
//! `futures` precedent): the session compiles them, and the
//! precompiled-kit lane folds them into the fingerprint-matched bundle
//! (the `kit_bundle` pin re-blesses with the set).
//!
//! Registered by `TurStdPlugin` (the kit-registration precedent: the
//! offering is standard surface, not an app-level choice), never by
//! core — the layering law. json itself carries ZERO host rows ("pure
//! rut, zero host fns" — the stdlib survey); only its two transitive
//! `*_host` deps install bodies, taken from `rut-std` verbatim.
//!
//! The offering is deliberately NOT a prelude: `json` is nameable
//! (`use json::{ … };`), never ambient — a module that never names it
//! pays nothing at its own compile surface.

use std::collections::BTreeMap;

const JSON_RUT: &str = include_str!("vendor/json.rut");
const STRBUILD_RUT: &str = include_str!("vendor/strbuild.rut");
const POUCH_RUT: &str = include_str!("vendor/pouch.rut");
const NMAPSET_RUT: &str = include_str!("vendor/nmapset.rut");
const STRBUILD_HOST_D_RUT: &str = include_str!("vendor/strbuild_host.d.rut");
const NMAP_HOST_D_RUT: &str = include_str!("vendor/nmap_host.d.rut");

/// Assemble the offered set: the four lib pkgs (source-mounted, dep
/// heads spelled per their manifests) + the two host pkgs (decl-lowered
/// from the vendored `.d.rut` trees, bodies installed at boot from
/// `rut-std`). Pushed through [`RutPkgCx::offered`] /
/// [`RutPkgCx::boot_pkgs`] by [`super::std::TurStdPlugin`]'s register.
pub fn push_offering(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let source =
        |name: &str, text: &str, deps: &[&str]| -> rut_driver::Mounted {
            rut_driver::Mounted::source(name, text)
                .manifest(rut_driver::Manifest::lib(name, deps))
        };
    cx.offered.push(source("json", JSON_RUT, &["strbuild", "pouch", "nmapset"]));
    cx.offered.push(source("strbuild", STRBUILD_RUT, &["strbuild_host"]));
    cx.offered.push(source("pouch", POUCH_RUT, &[]));
    cx.offered.push(source("nmapset", NMAPSET_RUT, &["nmap_host"]));
    // The two host pkgs: decl-lowered like the vendored `async_host`
    // (flat single-file surfaces — no mod tree).
    let host_decl = |spec: &str, text: &str, origin: &str| -> rut_driver::Mounted {
        let mut m = rut_driver::lower_decl_module(text, &BTreeMap::new(), origin)
            .unwrap_or_else(|e| panic!("the vendored {spec} decl always lowers: {e}"));
        m.spec = spec.to_string();
        m
    };
    cx.offered.push(host_decl(
        "strbuild_host",
        STRBUILD_HOST_D_RUT,
        "strbuild_host.d.rut",
    ));
    cx.offered.push(host_decl(
        "nmap_host",
        NMAP_HOST_D_RUT,
        "nmap_host.d.rut",
    ));
    // The bodies: rut-std's own rows, installed beside tur_host at boot.
    cx.boot_pkgs.push(rut_std::strbuild::pkg());
    cx.boot_pkgs.push(rut_std::nmap::pkg());
}
