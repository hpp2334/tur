use parley::layout::PositionedLayoutItem;
use parley::{Alignment, AlignmentOptions, GenericFamily, StyleProperty};
use tur_shared::{Color, ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::TextElement;
use super::text_layout::{TextGlyph, TextLayoutData, TextRunData};

const DEFAULT_COLOR: Color = Color::rgb(255, 255, 255);

fn color_to_brush(color: &Color) -> [u8; 4] {
    [color.r(), color.g(), color.b(), color.a()]
}

impl ElementLayout for TextElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        if self.content.is_empty() {
            self.cached_layout = None;
            return constraints.constrain(Size::ZERO);
        }

        let color = self.color.as_ref().unwrap_or(&DEFAULT_COLOR);
        let brush = color_to_brush(color);
        let (font_cx, text_layout_cx) = cx.text_layout_contexts();

        let mut builder = text_layout_cx.ranged_builder(font_cx, &self.content, 1.0);
        builder.push_default(StyleProperty::FontSize(self.font_size as f32));
        builder.push_default(StyleProperty::Brush(brush));
        builder.push_default(StyleProperty::from(GenericFamily::SansSerif));

        let mut layout = builder.build(&self.content);

        let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
            Some(constraints.max_width as f32)
        } else {
            None
        };
        layout.break_all_lines(max_width);
        layout.align(max_width, Alignment::Start, AlignmentOptions::default());

        let width = layout.width();
        let height = layout.height();

        let mut runs = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font().clone();
                let font_size = run.font_size();
                let normalized_coords = run.normalized_coords().to_vec();
                let style = glyph_run.style();

                let mut glyphs = Vec::new();
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                for glyph in glyph_run.glyphs() {
                    let gx = x + glyph.x;
                    let gy = y - glyph.y;
                    x += glyph.advance;
                    glyphs.push(TextGlyph {
                        id: glyph.id as u32,
                        x: gx,
                        y: gy,
                        advance: glyph.advance,
                    });
                }

                runs.push(TextRunData {
                    font,
                    font_size,
                    normalized_coords,
                    glyphs,
                    brush: style.brush,
                });
            }
        }

        self.cached_layout = Some(TextLayoutData {
            runs,
            _width: width,
            _height: height,
        });

        constraints.constrain(Size::new(width as f64, height as f64))
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
        if let Some(ref layout_data) = self.cached_layout {
            canvas.fill_text_layout(offset, layout_data);
        }
    }
}
