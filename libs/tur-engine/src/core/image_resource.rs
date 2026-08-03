//! Image-only resource storage for tur.
//!
//! `ImageResourceId` / `ImageResourceMap` / `ImageResource` live here as the
//! engine's paint/layout contract: renderers read `ImageResource.peniko_image`
//! to upload textures, layout reads `.natural_size`, and the
//! `Canvas::draw_image(ImageResourceId, ...)` paint contract is keyed by id.
//!
//! Image *production* (PNG/JPEG/SVG decode → `ImageResource`) lives in the
//! standalone `tur-image` crate (`tur_image::decode`), mirroring how
//! `extract_layout_data` lives in `tur-text` rather than in this contract
//! module. Fields on `ImageResource` are `pub` for the same reason
//! `TextLayoutData`'s are: the feature crate constructs the struct directly.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::core::layout::Size;
use vello_common::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageResourceId(u64);

impl ImageResourceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Decoded image ready to be uploaded to the renderer atlas. Constructed by
/// `tur-image::decode::{decode_image_bytes, decode_svg}` from raw PNG/JPEG/SVG
/// input; read here by the engine's layout (`.natural_size`) and renderers
/// (`.peniko_image`).
///
/// `Clone` is cheap — `ImageData` wraps an `Arc`-backed `Blob`, so cloning
/// just bumps a refcount. This lets the worker ship its image map to main
/// each frame without deep-copying pixel data.
#[derive(Clone)]
pub struct ImageResource {
    pub peniko_image: ImageData,
    pub natural_size: Size,
}

impl ImageResource {
    /// Build a resource from raw RGBA pixels. Used by the decode fns in
    /// `tur-image`; kept here so the constructor sits next to the type
    /// definition (the bytes-to-pixels path is engine-internal — no
    /// `image`/`resvg`/`usvg` deps required).
    pub fn from_rgba(raw: &[u8], width: u32, height: u32) -> Option<Self> {
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

#[derive(Default, Clone)]
pub struct ImageResourceMap {
    resources: HashMap<ImageResourceId, ImageResource>,
    next_id: u64,
}

impl fmt::Debug for ImageResourceMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageResourceMap")
            .field("count", &self.resources.len())
            .finish()
    }
}

impl ImageResourceMap {
    pub fn insert_image(&mut self, image: ImageResource) -> ImageResourceId {
        let id = ImageResourceId(self.next_id);
        self.next_id += 1;
        self.resources.insert(id, image);
        id
    }

    pub fn get_image(&self, id: ImageResourceId) -> Option<&ImageResource> {
        self.resources.get(&id)
    }

    /// Iterate over all registered image resources with their ids.
    pub fn iter_images(&self) -> impl Iterator<Item = (ImageResourceId, &ImageResource)> {
        self.resources.iter().map(|(id, img)| (*id, img))
    }

    /// Whether an image resource with the given id is registered.
    pub fn has_image(&self, id: ImageResourceId) -> bool {
        self.resources.contains_key(&id)
    }
}
