use tur_shared::{ComputedLayout, Constraints, ElementKind, Offset, Size};

use crate::render_object::{ChildLayout, ChildPaint, PaintContext, RenderObject};
use crate::RenderNodeId;

#[derive(Debug)]
pub struct TextRenderObject {
    pub content: String,
    pub font_size: f64,
    pub color: Option<String>,
}

impl TextRenderObject {
    pub fn from_props(props: &std::collections::HashMap<String, tur_element::PropValue>) -> Self {
        TextRenderObject {
            content: super::prop_str(props, "content").unwrap_or("").to_string(),
            font_size: super::prop_f64(props, "fontSize").unwrap_or(14.0),
            color: super::prop_str(props, "color").map(String::from),
        }
    }
}

impl RenderObject for TextRenderObject {
    fn kind(&self) -> ElementKind {
        ElementKind::Text
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[RenderNodeId],
        _child_layout: &mut dyn ChildLayout,
    ) -> Size {
        let char_width = self.font_size * 0.6;
        let line_height = self.font_size * 1.2;

        let max_width = constraints.max_width;
        let chars_per_line = if max_width.is_finite() && max_width > 0.0 {
            (max_width / char_width).max(1.0) as usize
        } else {
            self.content.len().max(1)
        };

        let lines = if chars_per_line > 0 && !self.content.is_empty() {
            (self.content.len() as f64 / chars_per_line as f64).ceil() as usize
        } else {
            1
        };

        let width = if self.content.is_empty() {
            0.0
        } else if max_width.is_finite() {
            let actual_chars = self.content.len().min(chars_per_line);
            actual_chars as f64 * char_width
        } else {
            self.content.len() as f64 * char_width
        };

        let height = lines as f64 * line_height;

        constraints.constrain(Size::new(width, height))
    }

    fn perform_layout_position(
        &mut self,
        _children: &[RenderNodeId],
        _child_layout: &mut dyn ChildLayout,
    ) {
    }

    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        _layout: &ComputedLayout,
        _children: &[RenderNodeId],
        _child_paint: &mut dyn ChildPaint,
    ) {
        if self.content.is_empty() {
            return;
        }
        let color = self.color.as_deref().unwrap_or("#ffffff");
        ctx.fill_text(offset, &self.content, self.font_size, color);
    }
}
