//! Image plugin — `Image` element + format decoders.
//!
//! Provides the `Image` element (`ImageElement` / `ImageView`), its layout +
//! paint implementations, and the format-specific decoders
//! (`decode_image_bytes` for PNG/JPEG, `decode_svg` for SVG strings).
//!
//! Elements materialize pure-Rust views (authored through the
//! `core::rut_runtime` image rows); resources are registered via
//! `InstanceContext::register_image` (worker keeps sizes only; the pixel
//! `Blob` ships to main via `HostMsg::UploadImage`).
//!
//! The engine retains only the paint/layout contract —
//! `crate::core::image_resource::{ImageResourceId, ImageManager,
//! ImageResource}` (pure-data struct with `pub` fields) — which
//! `Canvas::draw_image` consumes.

pub mod decode;
pub mod element;
pub mod handle;
pub mod layout;
pub(crate) mod rut_rows;
pub mod render;

/// Install the image family's `tur` host-pkg rows (the kit wraps them):
/// resource registration + the Image spec.
pub fn install_image(ctx: &mut crate::core::plugin::PluginRegisterContext) -> Result<(), crate::error::TurError> {
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_ext));
    Ok(())
}

pub use element::{ImageElement, ImageView};
