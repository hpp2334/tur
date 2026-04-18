use tur_render_tree::render_node::{RenderNode, RenderNodeId};
use tur_render_tree::RenderTree;
use tur_shared::{Offset, Size};
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

pub fn paint_tree(ctx: &mut PaintContext, tree: &RenderTree) {
    let root_id = match tree.root_id() {
        Some(id) => id,
        None => return,
    };
    paint_node(ctx, tree, root_id, Offset::ZERO);
}

fn paint_node(ctx: &mut PaintContext, tree: &RenderTree, id: RenderNodeId, parent_offset: Offset) {
    let node = match tree.get(id) {
        Some(n) => n,
        None => return,
    };

    let absolute_offset = parent_offset + node.computed_layout.offset;

    match node.kind {
        tur_element::ElementKind::Text => paint_text(ctx, node, absolute_offset),
        tur_element::ElementKind::Container => {
            paint_container(ctx, node, absolute_offset, node.computed_layout.size)
        }
        tur_element::ElementKind::SizedBox
        | tur_element::ElementKind::Column
        | tur_element::ElementKind::Row
        | tur_element::ElementKind::Expanded
        | tur_element::ElementKind::Stack
        | tur_element::ElementKind::Positioned => {}
    }

    for &child_id in &node.children {
        paint_node(ctx, tree, child_id, absolute_offset);
    }
}

fn paint_text(ctx: &mut PaintContext, node: &RenderNode, offset: Offset) {
    let content = node.text_content.as_deref().unwrap_or("");
    let _font_size = node.font_size.unwrap_or(14.0) as f32;
    let color = parse_color(node.color.as_deref().unwrap_or("#ffffff"));

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

fn paint_container(ctx: &mut PaintContext, node: &RenderNode, offset: Offset, size: Size) {
    let color = match &node.color {
        Some(c) => parse_color(c),
        None => return,
    };

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
