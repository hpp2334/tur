use parley::{Alignment, AlignmentOptions, GenericFamily, StyleProperty};
use tur_shared::{Color, ComputedLayout, Constraints, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::text_layout;

use super::element::InputElement;

const DEFAULT_COLOR: Color = Color::rgb(255, 255, 255);
const SELECTION_COLOR: Color = Color::rgba(0, 120, 215, 77);

fn color_to_brush(color: &Color) -> [u8; 4] {
    [color.r(), color.g(), color.b(), color.a()]
}

fn build_text_layout(
    content: &str,
    font_size: f64,
    color: &Color,
    constraints: &Constraints,
    cx: &mut LayoutContext,
) -> (text_layout::TextLayoutData, f32, f32) {
    let brush = color_to_brush(color);
    let (font_cx, text_layout_cx) = cx.text_layout_contexts();

    let mut builder = text_layout_cx.ranged_builder(font_cx, content, 1.0);
    builder.push_default(StyleProperty::FontSize(font_size as f32));
    builder.push_default(StyleProperty::Brush(brush));
    builder.push_default(StyleProperty::from(GenericFamily::SansSerif));

    let mut layout = builder.build(content);

    let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
        Some(constraints.max_width as f32)
    } else {
        None
    };
    layout.break_all_lines(max_width);
    layout.align(max_width, Alignment::Start, AlignmentOptions::default());

    text_layout::extract_layout_data(&mut layout, &[])
}

impl ElementLayout for InputElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let display_text = if self.content.is_empty() {
            self.placeholder.as_deref().unwrap_or("")
        } else {
            &self.content
        };

        if display_text.is_empty() {
            self.cached_layout = None;
            let height = self.font_size * 1.2;
            return constraints.constrain(Size::new(0.0, height));
        }

        let placeholder_color = Color::rgb(153, 153, 153);
        let color = if self.content.is_empty() {
            self.placeholder_color
                .as_ref()
                .unwrap_or(&placeholder_color)
        } else {
            self.color.as_ref().unwrap_or(&DEFAULT_COLOR)
        };

        let (layout_data, width, height) =
            build_text_layout(display_text, self.font_size, color, constraints, cx);

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for InputElement {
    fn type_name(&self) -> &'static str {
        "tur_input"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        _children: &[ElementNodeId],
        _paint_ctx: &PaintContext,
    ) {
        if let Some(ref layout_data) = self.cached_layout {
            if self.has_selection() {
                let (start, end) = self.selection_range();
                let x_start = layout_data.cursor_x_at(start);
                let x_end = layout_data.cursor_x_at(end);
                canvas.fill_geometry(
                    Offset::new(offset.x + x_start as f64, offset.y),
                    &Geometry::Rect(Size::new((x_end - x_start) as f64, layout.size.height)),
                    &SELECTION_COLOR,
                );
            }

            canvas.fill_text_layout(offset, layout_data);

            if self.focused && !self.has_selection() {
                let cursor_x = self
                    .cached_layout
                    .as_ref()
                    .map(|ld| ld.cursor_x_at(self.cursor_position) as f64)
                    .unwrap_or(0.0);
                let cursor_color = self
                    .cursor_color
                    .as_ref()
                    .or(self.color.as_ref())
                    .unwrap_or(&DEFAULT_COLOR);
                let height = layout.size.height;
                canvas.fill_geometry(
                    Offset::new(offset.x + cursor_x, offset.y),
                    &Geometry::Rect(Size::new(2.0, height)),
                    cursor_color,
                );
            }
        }
    }
}
