//! Image plugin — `Image` element + format decoders.
//!
//! Provides the `Image` element (`ImageElement` / `ImageView`), its layout +
//! paint implementations, the JS bridge fns (`Image`, `createImageResource`,
//! `createSvgResource`), and the format-specific decoders
//! (`decode_image_bytes` for PNG/JPEG, `decode_svg` for SVG strings).
//!
//! Installed into `tur:std` by `TurStdPlugin` via [`install_image`],
//! which returns the JS factory fns to be merged into `std_fns`. From JS's
//! perspective `Image` / `createImageResource` / `createSvgResource` ship as
//! part of `tur:std`.
//!
//! The engine retains only the paint/layout contract —
//! `crate::core::image_resource::{ImageResourceId, ImageResourceMap,
//! ImageResource}` (pure-data struct with `pub` fields) — which
//! `Canvas::draw_image` consumes. This plugin produces these structs from
//! raw bytes / SVG strings via [`decode`].

pub mod bridge;
pub mod decode;
pub mod element;
pub mod layout;
pub mod render;

pub use element::{ImageElement, ImageView};

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Wire image plugin into `tur:std`. Called by `TurStdPlugin`'s
/// `register` impl.
///
/// Side effects: none beyond returning the factory fns (no classes, no
/// subsystems — image rendering is fully synchronous and stateless from the
/// JS bridge's perspective).
///
/// Returns: the `Image` / `createImageResource` / `createSvgResource` factory
/// fns, which the caller merges into `std_fns` before
/// `register_module("tur:std", ...)`.
pub fn install_image(_ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    Ok(bridge::fns())
}
