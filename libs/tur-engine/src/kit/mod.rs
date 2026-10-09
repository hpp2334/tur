//! The kit — the authored element-construction surface.
//!
//! [`tur_kit_source`] is a rut SOURCE module spliced from the files under
//! `rut/tur_kit/` (one class per element over its family's `tur_host` pkg
//! rows: one method per prop, `.child` / `.children` appending, `.build()`
//! the only terminal). The file list + order IS the kit manifest's
//! `entry.lib` + `entry.libs` (`rut/tur_kit/rut.jsonc`) — the loader
//! splices base-first, then `libs` in listed order, newline-joined into
//! ONE module; the embed here mirrors that byte-for-byte, and the
//! `tur_host_decl` pin test diffs the two lists. The kit lives OUTSIDE
//! `core/` (the layering law: core owns mechanism, never elements) and is
//! registered as a compile-session prelude by the standard bundle
//! assembly — [`crate::builtin_plugins::std::TurStdPlugin`] pushes it
//! through the pkg-extension seam's `preludes` channel, so `use
//! tur_kit::{ Column, … }` resolves in every module the standard plugin
//! set compiles. Core never names it.

/// The kit's files, in the manifest's canonical splice order (base first,
/// then `entry.libs`) — the list mirrors `rut/tur_kit/rut.jsonc` and is
/// the pin test's diff target.
pub const TUR_KIT_FILES: [&str; 17] = [
    "handles.rut",
    "flags.rut",
    "dispatch.rut",
    "reactive.rut",
    "flex.rut",
    "box.rut",
    "stack.rut",
    "text.rut",
    "input.rut",
    "image.rut",
    "grid_table.rut",
    "scroll_lazy.rut",
    "gesture.rut",
    "focus.rut",
    "lifecycle.rut",
    "control.rut",
    "virtual_app.rut",
];

/// The spliced kit source — the files `\n`-joined in [`TUR_KIT_FILES`]
/// order (the same splice law the rut loader applies to a manifest's
/// `entry.libs`; the `*_RUT` consts below list the SAME files in the SAME
/// order).
pub fn tur_kit_source() -> String {
    [
        HANDLES_RUT, FLAGS_RUT, DISPATCH_RUT, REACTIVE_RUT, FLEX_RUT, BOX_RUT, STACK_RUT,
        TEXT_RUT, INPUT_RUT, IMAGE_RUT, GRID_TABLE_RUT, SCROLL_LAZY_RUT, GESTURE_RUT, FOCUS_RUT,
        LIFECYCLE_RUT, CONTROL_RUT, VIRTUAL_APP_RUT,
    ]
    .join("\n")
}

const HANDLES_RUT: &str = include_str!("../../../../rut/tur_kit/handles.rut");
const FLAGS_RUT: &str = include_str!("../../../../rut/tur_kit/flags.rut");
const DISPATCH_RUT: &str = include_str!("../../../../rut/tur_kit/dispatch.rut");
const REACTIVE_RUT: &str = include_str!("../../../../rut/tur_kit/reactive.rut");
const FLEX_RUT: &str = include_str!("../../../../rut/tur_kit/flex.rut");
const BOX_RUT: &str = include_str!("../../../../rut/tur_kit/box.rut");
const STACK_RUT: &str = include_str!("../../../../rut/tur_kit/stack.rut");
const TEXT_RUT: &str = include_str!("../../../../rut/tur_kit/text.rut");
const INPUT_RUT: &str = include_str!("../../../../rut/tur_kit/input.rut");
const IMAGE_RUT: &str = include_str!("../../../../rut/tur_kit/image.rut");
const GRID_TABLE_RUT: &str = include_str!("../../../../rut/tur_kit/grid_table.rut");
const SCROLL_LAZY_RUT: &str = include_str!("../../../../rut/tur_kit/scroll_lazy.rut");
const GESTURE_RUT: &str = include_str!("../../../../rut/tur_kit/gesture.rut");
const FOCUS_RUT: &str = include_str!("../../../../rut/tur_kit/focus.rut");
const LIFECYCLE_RUT: &str = include_str!("../../../../rut/tur_kit/lifecycle.rut");
const CONTROL_RUT: &str = include_str!("../../../../rut/tur_kit/control.rut");
const VIRTUAL_APP_RUT: &str = include_str!("../../../../rut/tur_kit/virtual_app.rut");

/// The kit as a compile pkg (spec `tur_kit`) — what the standard bundle
/// assembly pushes into [`crate::core::rut_runtime::RutPkgCx::preludes`].
pub fn tur_kit_pkg() -> rut_driver::Pkg {
    rut_driver::Pkg {
        spec: "tur_kit".to_string(),
        namespace: Some("tur_kit".to_string()),
        body: rut_driver::PkgBody::Source {
            text: tur_kit_source(),
            is_decl: false,
        },
        ..Default::default()
    }
}
