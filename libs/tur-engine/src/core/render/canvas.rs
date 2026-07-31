use std::fmt;

use crate::core::render::brush::{Brush, Color};
use crate::core::layout::{Geometry, Offset, Size};
use vello_common::kurbo::Affine;

use crate::core::image_resource::ImageResourceId;
use crate::core::text::text_layout::TextLayoutData;

pub trait Canvas: fmt::Debug {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, brush: &Brush);
    fn stroke_geometry(
        &mut self,
        offset: Offset,
        geometry: &Geometry,
        color: &Color,
        stroke_width: f64,
    );
    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, offset: Offset, layout: &TextLayoutData);
    fn draw_image(&mut self, resource_id: ImageResourceId, natural_size: Size, transform: Affine);
    fn draw_shadow(
        &mut self,
        offset: Offset,
        size: Size,
        color: &Color,
        border_radius: f64,
        blur: f64,
        shadow_offset: (f64, f64),
    );
    fn push_clip(&mut self, offset: Offset, size: Size);
    /// Push a clip layer defined by an arbitrary `Geometry` (rect, rounded
    /// rect, or circle) at the given local offset. The shape lives in this
    /// node's local space; the canvas transform stack (already positioning
    /// the node) applies. Used by `Container`'s `clipBehavior` to clip
    /// children to a rounded decoration shape — `push_clip` only handles
    /// plain rects (overflow clipping).
    fn push_clip_geometry(&mut self, offset: Offset, geometry: &Geometry);
    fn pop_clip(&mut self);
    /// Push an opacity layer: subsequent draws are alpha-composited with the
    /// given opacity (0.0..=1.0) until `pop_opacity` is called. Vello
    /// implements this as a clipped push_layer with a Solid alpha brush.
    fn push_opacity(&mut self, opacity: f32);
    fn pop_opacity(&mut self);
    /// Push a transform layer: subsequent draws are transformed by the given
    /// affine (in addition to any current canvas offset/transform) until
    /// `pop_transform` is called.
    fn push_transform(&mut self, transform: Affine);
    fn pop_transform(&mut self);
}

/// A `Canvas` that discards every draw. Used by `NoopRenderer` to drive the
/// paint walk (so paint-time outputs like cursor resolution still happen)
/// without producing any pixels.
#[derive(Debug, Default)]
pub struct NullCanvas;

impl Canvas for NullCanvas {
    fn fill_geometry(&mut self, _offset: Offset, _geometry: &Geometry, _brush: &Brush) {}
    fn stroke_geometry(
        &mut self,
        _offset: Offset,
        _geometry: &Geometry,
        _color: &Color,
        _stroke_width: f64,
    ) {
    }
    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, _offset: Offset, _layout: &TextLayoutData) {}
    fn draw_image(&mut self, _resource_id: ImageResourceId, _natural_size: Size, _transform: Affine) {}
    fn draw_shadow(
        &mut self,
        _offset: Offset,
        _size: Size,
        _color: &Color,
        _border_radius: f64,
        _blur: f64,
        _shadow_offset: (f64, f64),
    ) {
    }
    fn push_clip(&mut self, _offset: Offset, _size: Size) {}
    fn push_clip_geometry(&mut self, _offset: Offset, _geometry: &Geometry) {}
    fn pop_clip(&mut self) {}
    fn push_opacity(&mut self, _opacity: f32) {}
    fn pop_opacity(&mut self) {}
    fn push_transform(&mut self, _transform: Affine) {}
    fn pop_transform(&mut self) {}
}
