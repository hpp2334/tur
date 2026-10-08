//! The packed-color law at the row boundary: every `tur_host` color row takes a
//! `u64` packed **`0xRRGGBBAA`** — R in bits 31..24, A in bits 7..0 (the
//! layout `Color::from_str`'s `#RRGGBBAA` and the corpus's `0xFF0000FF` =
//! red both pin). The truth lives in `core::rut_runtime::color_of`; this
//! test pins it end-to-end at the `RenderCommand` level so a row-side
//! re-pack (e.g. to ARGB) can never land silently.
//!
//! Captured with a recording renderer and read straight out of the
//! `FillGeometry` op's brush — renderer-independent ground truth (no GPU).

use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::core::render::brush::Brush;
use tur_engine::core::render::{CanvasOp, RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

/// Renderer that stashes each frame's command batch.
struct RecordingRenderer {
    last: Rc<RefCell<Vec<RenderCommand>>>,
}

impl Renderer for RecordingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        *self.last.borrow_mut() = commands.to_vec();
    }
}

/// Channel bytes of every solid `FillGeometry` brush painted this frame.
fn fill_brushes(cmds: &[RenderCommand]) -> Vec<[u8; 4]> {
    let mut brushes = Vec::new();
    for cmd in cmds {
        let RenderCommand::Paint { ops, .. } = cmd;
        for op in ops {
            if let CanvasOp::FillGeometry { brush, .. } = op {
                if let Brush::SolidColor(c) = brush {
                    brushes.push([c.r(), c.g(), c.b(), c.a()]);
                }
            }
        }
    }
    brushes
}

/// `Container.color(0x11223344)` must paint exactly R:0x11, G:0x22, B:0x33,
/// A:0x44 — the packed value is `0xRRGGBBAA`, not ARGB.
#[test]
fn container_color_packs_rrggbbaa() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        100.0,
        100.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    app.load_rut_module(
        r#"

use tur_kit::{ Container, mount };


entry fn start() {
    mount(Container().width_height(40.0, 40.0).color(0x11223344u64).build());
}
"#,
    )
    .expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let brushes = fill_brushes(&last.borrow());
    assert_eq!(
        brushes,
        vec![[0x11, 0x22, 0x33, 0x44]],
        "the packed color u64 is 0xRRGGBBAA: R in bits 31..24, A in bits 7..0"
    );
}
