//! The kit — the authored element-construction surface.
//!
//! `rut/tur_kit` is a rut FILE-MODULE TREE: the root `mod.rut` declares
//! the domain subtrees and every leaf under them is its own module (one
//! class family per leaf — one method per prop, `.child` / `.children`
//! appending, `.build()` the only terminal, over each family's
//! `tur_host` pkg rows). The embed here hands the tree to the compiler
//! through [`rut_driver::Pkg::mods`] — the root as the pkg body, every
//! intermediate + leaf module as a [`rut_driver::ModSource`] row keyed
//! by mod path — and the `tur_host_decl` pin test diffs the embedded
//! set against the disk tree. The kit lives OUTSIDE `core/` (the
//! layering law: core owns mechanism, never elements) and is registered
//! as a compile-session prelude by the standard bundle assembly —
//! [`crate::builtin_plugins::std::TurStdPlugin`] pushes it through the
//! pkg-extension seam's `preludes` channel, so `use
//! tur_kit::layout::flex::{ Column }` resolves in every module the
//! standard plugin set compiles. Core never names it.

use rut_driver::{ModSource, Pkg, PkgBody};

/// The kit tree's root module — `rut/tur_kit/mod.rut` (decls only: the
/// `pub mod` edges).
pub const TUR_KIT_ROOT: &str = include_str!("../../../../rut/tur_kit/mod.rut");

const DISPATCH_RUT: &str = include_str!("../../../../rut/tur_kit/dispatch/mod.rut");
const FLAGS_RUT: &str = include_str!("../../../../rut/tur_kit/flags/mod.rut");
const HANDLES_RUT: &str = include_str!("../../../../rut/tur_kit/handles/mod.rut");
const REACTIVE_RUT: &str = include_str!("../../../../rut/tur_kit/reactive/mod.rut");

const CONTROL_FLOW_RUT: &str = include_str!("../../../../rut/tur_kit/control_flow/mod.rut");
const CONTROL_RUT: &str = include_str!("../../../../rut/tur_kit/control_flow/control/mod.rut");
const LIFECYCLE_RUT: &str = include_str!("../../../../rut/tur_kit/control_flow/lifecycle/mod.rut");

const GESTURE_RUT: &str = include_str!("../../../../rut/tur_kit/gesture/mod.rut");
const FOCUS_RUT: &str = include_str!("../../../../rut/tur_kit/gesture/focus/mod.rut");
const POINTER_RUT: &str = include_str!("../../../../rut/tur_kit/gesture/pointer/mod.rut");

const LAYOUT_RUT: &str = include_str!("../../../../rut/tur_kit/layout/mod.rut");
const BOX_RUT: &str = include_str!("../../../../rut/tur_kit/layout/box/mod.rut");
const FLEX_RUT: &str = include_str!("../../../../rut/tur_kit/layout/flex/mod.rut");
const GRID_TABLE_RUT: &str = include_str!("../../../../rut/tur_kit/layout/grid_table/mod.rut");
const STACK_RUT: &str = include_str!("../../../../rut/tur_kit/layout/stack/mod.rut");

const TEXT_RUT: &str = include_str!("../../../../rut/tur_kit/text/mod.rut");
const TEXT_CORE_RUT: &str = include_str!("../../../../rut/tur_kit/text/core/mod.rut");
const TEXT_INPUT_RUT: &str = include_str!("../../../../rut/tur_kit/text/input/mod.rut");

const IMAGE_RUT: &str = include_str!("../../../../rut/tur_kit/image/mod.rut");
const SCROLL_RUT: &str = include_str!("../../../../rut/tur_kit/scroll/mod.rut");
const VIRTUAL_APP_RUT: &str = include_str!("../../../../rut/tur_kit/virtual_app/mod.rut");

/// The kit's file-module tree: `(mod path, edge vis, text)` — the root
/// rides the pkg body (never a row); every declared child (the four
/// decls-only domain roots included) is a row here, keyed by mod path.
/// The `tur_host_decl` pin test diffs this set against the disk tree.
pub fn tur_kit_mods() -> Vec<(&'static str, rut_ast::ast::Vis, &'static str)> {
    use rut_ast::ast::Vis;
    vec![
        ("", Vis::Pub, TUR_KIT_ROOT),
        ("dispatch", Vis::Pkg, DISPATCH_RUT),
        ("flags", Vis::Pub, FLAGS_RUT),
        ("handles", Vis::Pub, HANDLES_RUT),
        ("reactive", Vis::Pub, REACTIVE_RUT),
        ("control_flow", Vis::Pub, CONTROL_FLOW_RUT),
        ("control_flow/control", Vis::Pub, CONTROL_RUT),
        ("control_flow/lifecycle", Vis::Pub, LIFECYCLE_RUT),
        ("gesture", Vis::Pub, GESTURE_RUT),
        ("gesture/focus", Vis::Pub, FOCUS_RUT),
        ("gesture/pointer", Vis::Pub, POINTER_RUT),
        ("layout", Vis::Pub, LAYOUT_RUT),
        ("layout/box", Vis::Pub, BOX_RUT),
        ("layout/flex", Vis::Pub, FLEX_RUT),
        ("layout/grid_table", Vis::Pub, GRID_TABLE_RUT),
        ("layout/stack", Vis::Pub, STACK_RUT),
        ("text", Vis::Pub, TEXT_RUT),
        ("text/core", Vis::Pub, TEXT_CORE_RUT),
        ("text/input", Vis::Pub, TEXT_INPUT_RUT),
        ("image", Vis::Pub, IMAGE_RUT),
        ("scroll", Vis::Pub, SCROLL_RUT),
        ("virtual_app", Vis::Pub, VIRTUAL_APP_RUT),
    ]
}

/// The kit as a compile pkg (spec `tur_kit`) — what the standard bundle
/// assembly pushes into [`crate::core::rut_runtime::RutPkgCx::preludes`].
/// The body is the root module; [`Pkg::mods`] carries the declared tree.
pub fn tur_kit_pkg() -> Pkg {
    let mut mods = std::collections::BTreeMap::new();
    for (path, vis, text) in tur_kit_mods() {
        if path.is_empty() {
            continue; // the root is the body, never a row
        }
        mods.insert(
            path.to_string(),
            ModSource {
                path: path.to_string(),
                vis,
                text: text.to_string(),
            },
        );
    }
    Pkg {
        spec: "tur_kit".to_string(),
        namespace: Some("tur_kit".to_string()),
        body: PkgBody::Source {
            text: TUR_KIT_ROOT.to_string(),
            is_decl: false,
        },
        mods,
        ..Default::default()
    }
}
