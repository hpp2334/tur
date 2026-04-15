use tur_layout::LayoutTree;
use tur_render_tree::RenderTree;
use tur_shared::Constraints;
use tur_vello_renderer::VelloRenderer;
use tur_widget::WidgetTree;
use vello::Scene;

#[allow(dead_code)]
pub fn render_frame(
    renderer: &mut VelloRenderer,
    widget_tree: &WidgetTree,
    width: f64,
    height: f64,
) -> Scene {
    let constraints = Constraints {
        min_width: 0.0,
        max_width: width,
        min_height: 0.0,
        max_height: height,
    };

    let mut layout_tree = LayoutTree::from_widget_tree(widget_tree);
    layout_tree.compute_layout(&constraints);

    let render_tree = RenderTree::from_layout_tree(&layout_tree, widget_tree);
    renderer.render_to_scene(&render_tree);

    renderer.scene().clone()
}
