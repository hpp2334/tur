use tur_shared::{Color, ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::TextElement;

const DEFAULT_COLOR: Color = Color::rgb(255, 255, 255);

impl ElementLayout for TextElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        _cx: &mut LayoutContext,
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

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for TextElement {
    fn type_name(&self) -> &'static str {
        "tur_text"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        _paint_ctx: &PaintContext,
    ) {
        if self.content.is_empty() {
            return;
        }
        let color = self.color.as_ref().unwrap_or(&DEFAULT_COLOR);
        canvas.fill_text(offset, &self.content, self.font_size, color);
    }
}
