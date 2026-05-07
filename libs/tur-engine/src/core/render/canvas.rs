use std::fmt;

use tur_shared::{Color, Geometry, Offset};
use vello::kurbo::Affine;
use vello::peniko::Image;

use crate::elements::text::text_layout::TextLayoutData;

pub trait Canvas: fmt::Debug {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, color: &Color);
    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, offset: Offset, layout: &TextLayoutData);
    fn draw_image(&mut self, image: &Image, transform: Affine);
}
