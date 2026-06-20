use std::sync::Arc;

use tur_shared::Size;
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

pub struct ImageResource {
    pub peniko_image: ImageData,
    pub natural_size: Size,
}

impl ImageResource {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let img = image::load_from_memory(data).ok()?;
        let rgba = img.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();
        let raw: Vec<u8> = rgba.into_raw();
        Self::from_rgba(&raw, width, height)
    }

    /// Rasterise an SVG string to an RGBA image at the SVG's declared size
    /// (i.e. the dimensions encoded in the document, e.g. `width="24"` or the
    /// `viewBox`). The output is stored as a vello `ImageData` so it flows
    /// through the existing image rendering pipeline — `Svg` is just another
    /// kind of image at runtime.
    pub fn from_svg_str(svg: &str) -> Option<Self> {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
        let size = tree.size();
        let pixmap_size = size.to_int_size();
        let width = pixmap_size.width();
        let height = pixmap_size.height();
        if width == 0 || height == 0 {
            return None;
        }
        let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        Self::from_rgba(pixmap.data(), width, height)
    }

    fn from_rgba(raw: &[u8], width: u32, height: u32) -> Option<Self> {
        if raw.len() != (width as usize) * (height as usize) * 4 {
            return None;
        }
        let blob = Blob::new(Arc::new(raw.to_vec().into_boxed_slice()));
        let peniko_image = ImageData {
            data: blob,
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width,
            height,
        };
        Some(ImageResource {
            peniko_image,
            natural_size: Size::new(width as f64, height as f64),
        })
    }
}
