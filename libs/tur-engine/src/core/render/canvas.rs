use std::fmt;

use tur_shared::{Color, Geometry, Offset};

pub trait Canvas: fmt::Debug {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, color: &Color);
    fn fill_text(&mut self, offset: Offset, text: &str, font_size: f64, color: &Color);
}
