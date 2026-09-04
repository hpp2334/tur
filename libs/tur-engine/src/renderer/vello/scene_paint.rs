//! Helpers shared by the wgpu and WebGL vello-hybrid backends.

use std::collections::HashMap;

use crate::core::image_resource::ImageResourceId;
use crate::core::render::RenderCommand;
use crate::core::render::brush::Color;
use crate::core::render::play_commands;
use crate::renderer::vello::paint_context::{VelloPaintContext, to_peniko_color};
use vello_common::kurbo::{Affine, Rect};
use vello_common::paint::{ImageId, PaintType};
use vello_common::peniko::Fill;
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

/// Paint a flat command batch (from the worker) into the scene.
///
/// Resets the scene, fills the base (background) color, seeds the dpr root
/// transform, then plays the commands back via [`play_commands`]. Each
/// [`RenderCommand::Paint`] wraps its ops in `notify_node_entry` /
/// `notify_node_exit` so the `VelloPaintContext` composes the per-node
/// absolute affine.
///
/// Image upload (backend-specific) must be performed by the caller *before*
/// calling this.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_commands_to_scene(
    scene: &mut Scene,
    resources: &mut Resources,
    image_uploads: &HashMap<ImageResourceId, ImageId>,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
    base_color: &Color,
    commands: &[RenderCommand],
) {
    scene.reset();
    paint_base_background(scene, physical_width, physical_height, base_color);
    let mut ctx = VelloPaintContext::new(scene, resources, Affine::scale(dpr), image_uploads);
    play_commands(&mut ctx, commands);
}

/// Fill the scene with the base color as the first element.
///
/// The hybrid renderer clears the surface to transparent, so this draws the
/// renderer's configured base color (opaque white by default) in physical
/// pixels with an identity transform. A translucent base color composites
/// over the transparent clear.
fn paint_base_background(
    scene: &mut Scene,
    physical_width: u32,
    physical_height: u32,
    base_color: &Color,
) {
    scene.set_transform(Affine::IDENTITY);
    scene.set_paint(PaintType::Solid(to_peniko_color(base_color)));
    scene.set_fill_rule(Fill::NonZero);
    scene.fill_rect(&Rect::new(
        0.0,
        0.0,
        physical_width as f64,
        physical_height as f64,
    ));
}
