use tur_layout::LayoutTree;
use tur_render_tree::RenderTree;
use tur_shared::Constraints;
use tur_vello_renderer::VelloRenderer;
use vello::Scene;

#[allow(dead_code)]
pub fn render_frame(renderer: &mut VelloRenderer, width: f64, height: f64) -> Scene {
    let constraints = Constraints {
        min_width: 0.0,
        max_width: width,
        min_height: 0.0,
        max_height: height,
    };

    let tree = tur_boajs::widget_tree();
    let tree_guard = tree.read().unwrap();

    let mut layout_tree = LayoutTree::from_widget_tree(&tree_guard);
    layout_tree.compute_layout(&constraints);

    let render_tree = RenderTree::from_layout_tree(&layout_tree, &tree_guard);
    renderer.render_to_scene(&render_tree);

    renderer.scene().clone()
}
