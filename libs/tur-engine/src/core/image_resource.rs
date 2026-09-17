//! Image-only resource storage for tur.
//!
//! `ImageResourceId` / `ImageResourceMap` / `ImageResource` live here as the
//! engine's paint/layout contract: renderers read `ImageResource.peniko_image`
//! to upload textures, layout reads `.natural_size`, and the
//! `Canvas::draw_image(ImageResourceId, ...)` paint contract is keyed by id.
//!
//! Ownership is split across the worker/main boundary:
//!
//! - **Worker side** — [`ImageManager`]: only the `natural_size` per id
//!   (plus the next-id counter). Layout + paint read sizes from it; the
//!   pixel `Blob` never lives on the worker across a frame boundary (it is
//!   shipped to main via `HostMsg::UploadImage`).
//! - **Main side** — [`ImageResourceMap`]: the full `ImageResource` (with its
//!   Arc-backed pixel `Blob`) per id, retained for context-loss re-upload.
//!   Main inserts under the worker-assigned id via
//!   [`ImageResourceMap::insert_with_id`] and uploads into the GPU atlas.
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

/// Base of the host-minted id range. Worker-minted ids (`ImageManager::allocate`)
/// count up from 0; ids minted host-side (`HostBackend::register_image`) count
/// DOWN from this base. Two properties fall out:
///
/// - The ranges are disjoint by construction (the worker would need 2^53
///   allocations to collide — guarded by a `debug_assert` in `allocate`).
/// - Every host id is ≤ 2^53, i.e. exactly representable as an f64 — the
///   precision the id crosses the JS number boundary with (ids round-trip as
///   plain JS numbers via `imageResourceHandle` / `resourceId`). A base above
///   2^53 would alias early host ids in JS.
pub const HOST_IMAGE_ID_BASE: u64 = 1 << 53;

impl ImageResourceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Whether this id was minted host-side (in the [`HOST_IMAGE_ID_BASE`]
    /// range) rather than by the worker's `ImageManager`.
    pub fn is_host_minted(self) -> bool {
        self.0 >= HOST_IMAGE_ID_BASE
    }
}

/// Decoded image ready to be uploaded to the renderer atlas. Constructed by
/// `tur-image::decode::{decode_image_bytes, decode_svg}` from raw PNG/JPEG/SVG
/// input; read here by the engine's layout (`.natural_size`) and renderers
/// (`.peniko_image`).
///
/// `Clone` is cheap — `ImageData` wraps an `Arc`-backed `Blob`, so cloning
/// just bumps a refcount. This lets the worker stage decoded images for the
/// one-way `HostMsg::UploadImage` ship to main without deep-copying pixel
/// data.
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

/// Worker-side image metadata: just the natural size (layout + paint read
/// the size; the pixel `Blob` lives on the host thread). One entry per
/// `createImageResource` / `createSvgResource` — inserted by
/// `ImageManager::allocate` at decode time. Wrapped in a struct (not
/// a bare `Size`) so future metadata fields can be added without rippling
/// through every read site.
#[derive(Debug, Clone, Copy)]
pub struct ImageMetadata {
    pub size: Size,
}

/// Worker-side image state: the natural-size map plus the next-id counter.
/// Both pieces always change together (each `register_image` allocates an id
/// and records its size), so they live behind one `Rc<RefCell<ImageManager>>`
/// on the worker instead of two separate cells. Layout + paint borrow this
/// immutably to read sizes; the JS bridge mutates it via [`Self::allocate`].
pub struct ImageManager {
    metadata: HashMap<ImageResourceId, ImageMetadata>,
    next_id: u64,
}

impl Default for ImageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageManager {
    pub fn new() -> Self {
        Self {
            metadata: HashMap::new(),
            next_id: 0,
        }
    }

    /// Assign the next worker-side id, record the image's natural size, and
    /// return the id. The pixel `Blob` is NOT stored here — the caller ships
    /// it to main via `HostMsg::UploadImage` under the returned id. This is
    /// the single mutation point: id allocation + size recording are atomic
    /// so layout + paint can never observe a stale size for an id the bridge
    /// already handed out.
    pub fn allocate(&mut self, image: &ImageResource) -> ImageResourceId {
        debug_assert!(
            self.next_id < HOST_IMAGE_ID_BASE,
            "worker image ids crossed into the host-minted range (2^53 allocations?!)"
        );
        let id = ImageResourceId::new(self.next_id);
        self.next_id += 1;
        self.metadata.insert(
            id,
            ImageMetadata {
                size: image.natural_size,
            },
        );
        id
    }

    /// Record metadata under an already-minted id — the worker-side receipt
    /// for host-registered images (`WorkerMsg::RegisterImageMetadata`): the
    /// host mints the id, retains the pixel Blob, and ships only the natural
    /// size across. Layout + paint then read the size exactly as they do for
    /// worker-minted ids.
    pub fn insert_with_id(&mut self, id: ImageResourceId, metadata: ImageMetadata) {
        self.metadata.insert(id, metadata);
    }

    /// Natural-size lookup for layout (`get_image_natural_size`) + paint
    /// (`get_image_size`). Returns the [`ImageMetadata`] so future fields
    /// extend without touching the signature.
    pub fn get(&self, id: ImageResourceId) -> Option<&ImageMetadata> {
        self.metadata.get(&id)
    }
}

impl fmt::Debug for ImageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageManager")
            .field("count", &self.metadata.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}

#[derive(Default, Clone)]
pub struct ImageResourceMap {
    resources: HashMap<ImageResourceId, ImageResource>,
}

impl fmt::Debug for ImageResourceMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageResourceMap")
            .field("count", &self.resources.len())
            .finish()
    }
}

impl ImageResourceMap {
    /// Insert an image under the worker-assigned id. Ids are the worker's
    /// authority (`TurInstanceContext::register_image` assigns them); main only
    /// stores what the worker ships via `HostMsg::UploadImage`.
    pub fn insert_with_id(&mut self, id: ImageResourceId, image: ImageResource) {
        self.resources.insert(id, image);
    }

    pub fn get_image(&self, id: ImageResourceId) -> Option<&ImageResource> {
        self.resources.get(&id)
    }

    /// Iterate over all retained image resources with their ids (main side —
    /// the worker never holds pixels, so this map is main-owned).
    pub fn iter_images(&self) -> impl Iterator<Item = (ImageResourceId, &ImageResource)> {
        self.resources.iter().map(|(id, img)| (*id, img))
    }

    /// Whether an image resource with the given id is registered.
    pub fn has_image(&self, id: ImageResourceId) -> bool {
        self.resources.contains_key(&id)
    }

    /// Whether no image resources are retained.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}
