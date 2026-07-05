use tur_shared::{Brush, Color, ComputedLayout, Geometry, Offset, Size};

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::render::{Canvas, ElementRender, PaintContext};

use super::element::{ScrollbarElement, MIN_THUMB};

/// Default thumb color — a semi-transparent neutral gray.
const DEFAULT_THUMB_COLOR: Color = Color::rgba(130, 130, 130, 160);

impl ElementRender for ScrollbarElement {
    fn type_name(&self) -> &'static str {
        "tur_scrollbar"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        _children: &[ElementNodeId],
        _paint_ctx: &PaintContext,
    ) {
        let (_, scroll_offset, max_extent, viewport) = match self.metrics() {
            Some(v) => v,
            None => return,
        };
        // Content fits the viewport — no thumb.
        if max_extent <= 0.0 {
            return;
        }
        let track_w = layout.size.width;
        let track_h = layout.size.height;
        if track_w <= 0.0 || track_h <= 0.0 {
            return;
        }

        let thumb_h = Self::thumb_height(track_h, max_extent, viewport).max(MIN_THUMB).min(track_h);
        let thumb_top = if max_extent > 0.0 {
            (scroll_offset / max_extent) * (track_h - thumb_h)
        } else {
            0.0
        };

        let p = &self.painting;
        // Optional track background — painted behind the thumb. Typically
        // wired reactively to a hover state from the JS side (transparent
        // when idle, light-gray when hovered).
        if let Some(track_brush) = p.track_color.as_ref() {
            let track_geometry = Geometry::Rect(Size::new(track_w, track_h));
            canvas.fill_geometry(offset, &track_geometry, track_brush);
        }

        let brush = p
            .color
            .clone()
            .unwrap_or(Brush::SolidColor(DEFAULT_THUMB_COLOR));

        let radius = p
            .thumb_radius
            .map(|r| r.min(track_w / 2.0).min(thumb_h / 2.0))
            .unwrap_or((track_w / 2.0).min(thumb_h / 2.0));

        let geometry = Geometry::RoundedRect {
            size: Size::new(track_w, thumb_h),
            radius,
        };
        canvas.fill_geometry(
            Offset::new(offset.x, offset.y + thumb_top),
            &geometry,
            &brush,
        );
    }
}
