use tracing;
use tur_vello_renderer::VelloRenderer;
use vello::Scene;

pub fn render_frame(renderer: &mut VelloRenderer, width: f64, height: f64) -> Scene {
    let constraints = tur_widget::Constraints {
        min_width: 0.0,
        max_width: width,
        min_height: 0.0,
        max_height: height,
    };

    let tree = tur_boajs::widget_tree();
    let mut tree_guard = tree.write().unwrap();
    renderer.render_to_scene(&mut tree_guard, &constraints);

    renderer.scene().clone()
}
