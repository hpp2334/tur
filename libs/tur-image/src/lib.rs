//! Image feature library for tur.
//!
//! Provides the `Image` element (`ImageElement` / `ImageView`), its layout +
//! paint implementations, the JS bridge fns (`Image`, `createImageResource`,
//! `createSvgResource`), and the format-specific decoders
//! (`decode_image_bytes` for PNG/JPEG, `decode_svg` for SVG strings).
//!
//! Like `tur-text`, this crate is **not** a plugin. It is installed into
//! `builtin:tur/std` by `TurStdPlugin` via [`install_image_feature`], which
//! returns the JS factory fns to be merged into `std_fns`. From JS's
//! perspective `Image` / `createImageResource` / `createSvgResource` ship as
//! part of `builtin:tur/std`.
//!
//! The engine retains only the paint/layout contract —
//! `tur_engine::core::image_resource::{ImageResourceId, ImageResourceMap,
//! ImageResource}` (pure-data struct with `pub` fields) — which
//! `Canvas::draw_image` consumes. tur-image produces these structs from raw
//! bytes / SVG strings via [`decode`].

pub mod bridge;
pub mod decode;
pub mod element;
pub mod layout;
pub mod render;

pub use element::{ImageElement, ImageView};

use tur_engine::core::bridge::helpers::FnEntry;
use tur_engine::core::plugin::PluginContext;
use tur_engine::error::TurError;

/// Wire image feature into `builtin:tur/std`. Called by `TurStdPlugin`'s
/// `register` impl — image is not a separate plugin, it's a feature installed
/// into the std module.
///
/// Side effects: none beyond returning the factory fns (no classes, no
/// subsystems — image rendering is fully synchronous and stateless from the
/// JS bridge's perspective).
///
/// Returns: the `Image` / `createImageResource` / `createSvgResource` factory
/// fns, which the caller merges into `std_fns` before
/// `register_module("builtin:tur/std", ...)`.
pub fn install_image_feature(_ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    Ok(bridge::fns())
}
