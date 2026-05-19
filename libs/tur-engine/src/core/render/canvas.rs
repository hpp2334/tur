use std::fmt;

use tur_shared::{Brush, Color, Geometry, Offset, Size};
use vello::kurbo::Affine;
use vello::peniko::ImageData;

use crate::elements::text::text_layout::TextLayoutData;

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
    fn draw_image(&mut self, image: &ImageData, transform: Affine);
    fn draw_shadow(
        &mut self,
        offset: Offset,
        size: Size,
        color: &Color,
        border_radius: f64,
        blur: f64,
        shadow_offset: (f64, f64),
    );
}
