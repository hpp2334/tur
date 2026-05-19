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
        let blob = Blob::new(Arc::new(raw.into_boxed_slice()));
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
