use std::fmt;

use tur_shared::{Constraints, Offset, Size};

use crate::core::elements::ElementTree;
use crate::core::traits::ElementNodeId;

pub trait ChildLayout {
    fn layout_child(&mut self, child_id: ElementNodeId, constraints: &Constraints) -> Size;
    fn set_child_offset(&mut self, child_id: ElementNodeId, offset: Offset);
    fn set_child_offset_self(&mut self, offset: Offset);
    fn get_child_type_name(&self, child_id: ElementNodeId) -> &'static str;
}

pub trait ChildPaint {
    fn paint_child(
        &mut self,
        child_id: ElementNodeId,
        ctx: &mut dyn PaintContext,
        parent_offset: Offset,
    );
}

pub trait PaintContext: fmt::Debug {
    fn fill_rect(&mut self, offset: Offset, size: Size, color: &str);
    fn fill_text(&mut self, offset: Offset, text: &str, font_size: f64, color: &str);
}

pub trait Renderer {
    fn render(&mut self, tree: &ElementTree);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
