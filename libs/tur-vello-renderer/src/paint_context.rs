use tur_widget::layout::{Offset, Size};
use tur_widget::{WidgetKind, WidgetTree};
use vello::kurbo::Affine;
use vello::peniko::{Brush, Color, Fill};
use vello::Scene;

pub struct PaintContext<'a> {
    scene: &'a mut Scene,
}

impl<'a> PaintContext<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        PaintContext { scene }
    }

    pub fn scene(&self) -> &Scene {
        self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        self.scene
    }
}

pub fn paint_tree(ctx: &mut PaintContext, tree: &WidgetTree) {
    let root_id = match tree.root_id() {
        Some(id) => id,
        None => return,
    };
    paint_node(ctx, tree, root_id, Offset::ZERO);
}

fn paint_node(ctx: &mut PaintContext, tree: &WidgetTree, id: u64, parent_offset: Offset) {
    let node = match tree.get(id) {
        Some(n) => n,
        None => return,
    };

    let computed = match &node.computed_layout {
        Some(cl) => cl,
        None => return,
    };

    let absolute_offset = parent_offset + computed.offset;

    match node.kind {
        WidgetKind::Text => paint_text(ctx, node, absolute_offset, computed.size),
        WidgetKind::Container => paint_container(ctx, node, absolute_offset, computed.size),
        WidgetKind::SizedBox
        | WidgetKind::Column
        | WidgetKind::Row
        | WidgetKind::Expanded
        | WidgetKind::Stack
        | WidgetKind::Positioned => {}
    }

    for &child_id in &node.children {
        paint_node(ctx, tree, child_id, absolute_offset);
    }
}

fn paint_text(ctx: &mut PaintContext, node: &tur_widget::WidgetNode, offset: Offset, _size: Size) {
    let content = node.prop_str("content").unwrap_or("");
    let _font_size = node.prop_f64("fontSize").unwrap_or(14.0) as f32;
    let color = parse_color(node.prop_str("color").unwrap_or("#ffffff"));

    if content.is_empty() {
        return;
    }

    let transform = Affine::translate((offset.x, offset.y));
    ctx.scene.fill(
        Fill::NonZero,
        transform,
        &Brush::Solid(color),
        None,
        &vello::kurbo::Rect::new(0.0, 0.0, 0.0, 0.0),
    );
}

fn paint_container(
    ctx: &mut PaintContext,
    node: &tur_widget::WidgetNode,
    offset: Offset,
    size: Size,
) {
    let color = match node.prop_str("color") {
        Some(c) => parse_color(c),
        None => return,
    };

    let _padding = node.prop_f64("padding").unwrap_or(0.0);

    let transform = Affine::translate((offset.x, offset.y));
    ctx.scene.fill(
        Fill::NonZero,
        transform,
        &Brush::Solid(color),
        None,
        &vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height),
    );
}

fn parse_color(s: &str) -> Color {
    let hex = s.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Color::from_rgba8(r, g, b, a)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, 255)
        }
        _ => Color::from_rgba8(255, 255, 255, 255),
    }
}
