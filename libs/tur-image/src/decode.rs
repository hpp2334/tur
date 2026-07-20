//! Format-specific image decoders. Each produces an `ImageResource` (the
//! engine's paint/layout contract type) from raw input. The PNG/JPEG path
//! uses the `image` crate; the SVG path uses `usvg` + `resvg` to rasterise
//! the document to RGBA at the SVG's declared size. Both end up as
//! premultiplied RGBA8 stored inside a vello `ImageData`.

use tur_engine::core::image_resource::ImageResource;

/// Decode a PNG or JPEG byte buffer into an `ImageResource`.
pub fn decode_image_bytes(data: &[u8]) -> Option<ImageResource> {
    let img = image::load_from_memory(data).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let raw: Vec<u8> = rgba.into_raw();
    ImageResource::from_rgba(&raw, width, height)
}

/// Rasterise an SVG string to an RGBA `ImageResource` at the SVG's declared
/// size (i.e. the dimensions encoded in the document, e.g. `width="24"` or
/// the `viewBox`). The output is stored as a vello `ImageData` so it flows
/// through the existing image rendering pipeline — `Svg` is just another
/// kind of image at runtime.
pub fn decode_svg(svg: &str) -> Option<ImageResource> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let pixmap_size = size.to_int_size();
    let width = pixmap_size.width();
    let height = pixmap_size.height();
    if width == 0 || height == 0 {
        return None;
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
    ImageResource::from_rgba(pixmap.data(), width, height)
}
