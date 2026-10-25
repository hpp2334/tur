//! SVG paint pins — the lane the playground journey exercised but no test
//! proved: an authored-SVG `Image` must actually reach the frame's paint
//! ops, both on a plain instance and through a child virtual-app instance
//! (the playground's shape — cases run as `VirtualApp` children whose
//! batches replay into the parent's frame with re-keyed image resources).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tur_engine::core::render::{CanvasOp, RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

/// A simple authored SVG (an indigo disc — the same authoring lane as the
/// github-viewer octocat / todolist check glyph).
const SVG_MARK: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='#4f46e5'><circle cx='12' cy='12' r='11'/></svg>";

fn plain_src() -> String {
    r#"
use tur_kit::handles::{ mount };
use tur_kit::image::{ Image, image_resource_svg };
use tur_kit::layout::box::{ Container };
use tur_kit::flags::{ Align };

entry fn start() -> u64 {
    let res = image_resource_svg("<SVG>");
    let img = Image(res).width(24.0).height(24.0).query_key("mark").build();
    mount(Container().width_height(40.0, 40.0).alignment(Align::Center).child(img).build());
    return res;
}
"#
    .replace("<SVG>", SVG_MARK)
}

/// The child's module: ONLY the SVG image (the parent mounts no image of
/// its own, so any DrawImage in the parent's frame is the child's, riding
/// the replay + remap lane). Eager boot (`entry fn start`).
fn child_src() -> String {
    r#"
use tur_kit::handles::{ mount };
use tur_kit::image::{ Image, image_resource_svg };
use tur_kit::layout::box::{ Container };
use tur_kit::flags::{ Align };

entry fn start() {
    let res = image_resource_svg("<SVG>");
    let img = Image(res).width(24.0).height(24.0).query_key("child-mark").build();
    mount(Container().width_height(100.0, 100.0).alignment(Align::Center).child(img).build());
}
"#
    .replace("<SVG>", SVG_MARK)
}

fn parent_module() -> String {
    let escaped = child_src()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!(
        r#"
use tur_kit::handles::{{ mount }};
use tur_kit::virtual_app::{{ VirtualApp, virtual_app_controller, virtual_app_source }};

entry fn start() {{
    let src = virtual_app_source("{escaped}");
    let ctrl = virtual_app_controller(src);
    let host = VirtualApp().controller(ctrl).width_height(200.0, 200.0).build();
    mount(host);
}}
"#
    )
}

/// Captures every painted `CanvasOp` into a shared buffer.
struct OpsRenderer {
    ops: Rc<RefCell<Vec<CanvasOp>>>,
}

impl Renderer for OpsRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        for cmd in commands {
            if let RenderCommand::Paint { ops, .. } = cmd {
                self.ops.borrow_mut().extend(ops.iter().cloned());
            }
        }
    }
    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    fn resize(&mut self, _w: u32, _h: u32, _dpr: f64) {}
    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}

fn attach_capturing(app: &TurTestApp, w: u32, h: u32) -> Rc<RefCell<Vec<CanvasOp>>> {
    let ops = Rc::new(RefCell::new(Vec::new()));
    app.attach_renderer(Box::new(OpsRenderer { ops: ops.clone() }), w, h, 1.0);
    ops
}

fn draw_image_ops(ops: &[CanvasOp]) -> Vec<&CanvasOp> {
    ops.iter()
        .filter(|op| matches!(op, CanvasOp::DrawImage { .. }))
        .collect()
}

/// A plain instance paints the authored-SVG image: the minted resource's
/// DrawImage op is in the frame.
#[test]
fn svg_image_paints_on_a_plain_instance() {
    let mut app = TurTestApp::new(100.0, 100.0).unwrap();
    app.load_rut_module(&plain_src()).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let res = app.rut_start_answer();
    let ops = attach_capturing(&app, 100, 100);
    app.wait_for_timeout(Duration::ZERO);

    let buf = ops.borrow();
    let drawn = draw_image_ops(&buf);
    assert!(
        drawn.iter().any(|op| match op {
            CanvasOp::DrawImage { resource_id, .. } => resource_id.as_u64() == res,
            _ => false,
        }),
        "the frame must carry a DrawImage for the minted SVG resource {res} (ops: {:?})",
        drawn.len()
    );
}

/// The playground shape: the SVG image lives in a CHILD virtual-app
/// instance; its frame replays into the parent's with re-keyed image
/// resources. The parent (which mounts no image of its own) must paint a
/// DrawImage once the child's first frame lands.
#[test]
fn svg_image_paints_through_a_child_virtual_app() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(&parent_module()).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let ops = attach_capturing(&app, 200, 200);

    // The child boots on the virtual-pool worker (real time); pump until
    // its replayed frame lands in the parent's paint ops.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        app.wait_for_timeout(Duration::ZERO);
        if !draw_image_ops(&ops.borrow()).is_empty() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the child's SVG DrawImage never replayed into the parent's frame"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
