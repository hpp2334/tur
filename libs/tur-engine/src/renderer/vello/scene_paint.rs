//! Helpers shared by the wgpu and WebGL vello-hybrid backends.

use std::collections::HashMap;

use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::resource::{ResourceId, ResourceMap};
use crate::core::shell::PaintShell;
use crate::renderer::vello::paint_context::VelloPaintContext;
use vello_common::kurbo::{Affine, Rect};
use vello_common::paint::{ImageId, PaintType};
use vello_common::peniko::{Color, Fill};
use vello_hybrid::{Resources, Scene};

/// Create a new [`Scene`] sized to the given physical pixel dimensions.
///
/// The hybrid `Scene` is created with fixed pixel dimensions, so it must be
/// recreated on resize.
pub(crate) fn new_scene(physical_width: u32, physical_height: u32) -> Scene {
    Scene::new(
        physical_width.min(u16::MAX as u32) as u16,
        physical_height.min(u16::MAX as u32) as u16,
    )
}

/// Paint the element tree into the scene.
///
/// This resets the scene, fills an opaque white background (the engine's
/// default base color), then paints the tree via [`VelloPaintContext`]. The
/// dpr scale is seeded as the paint context's root transform so it is baked
/// into every draw call.
///
/// Image upload (backend-specific) must be performed by the caller *before*
/// calling this, populating `image_uploads` with the [`ImageId`] for each
/// registered image resource. Both backends only support
/// `ImageSource::OpaqueId`, so every image must be uploaded to the atlas first.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_tree_to_scene(
    scene: &mut Scene,
    resources: &mut Resources,
    image_uploads: &HashMap<ResourceId, ImageId>,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
    tree: &NodeTreeData,
    focused_node_id: Option<ElementNodeId>,
    resource_map: &ResourceMap,
    shell: PaintShell<'_>,
) {
    scene.reset();
    // The hybrid renderer clears the surface to transparent, so paint an
    // opaque white background as the first element (the engine's default base
    // color). It is drawn in physical pixels with an identity transform.
    scene.set_transform(Affine::IDENTITY);
    scene.set_paint(PaintType::Solid(Color::from_rgba8(255, 255, 255, 255)));
    scene.set_fill_rule(Fill::NonZero);
    scene.fill_rect(&Rect::new(
        0.0,
        0.0,
        physical_width as f64,
        physical_height as f64,
    ));
    // Paint the tree directly into the main scene (vello_hybrid has no
    // `Scene::append`). The dpr scale is seeded as the paint context's root
    // transform so it is baked into every draw call.
    let mut ctx = VelloPaintContext::new(scene, resources, Affine::scale(dpr), image_uploads);
    tree.paint(&mut ctx, focused_node_id, resource_map, shell);
}
