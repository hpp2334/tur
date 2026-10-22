//! The border stroke-path's corner geometry — Flutter-deflate parity.
//!
//! An inside border's stroke path deflates the box SIZE by the border
//! width; its RADIUS must deflate by half the width so the stroke's
//! outer edge coincides exactly with the fill's rounded boundary (same
//! arc center, outer radius `r`). Keeping the fill's radius mis-centers
//! the corner arcs and the border's outer edge pokes outside the fill's
//! rounded corner — the "border ears" the todolist checkbox showed.
//! `Outside` symmetrically inflates; `Center` straddles as-is.
//!
//! Pinned at the command level (the recorded `CanvasOp::StrokeGeometry`
//! shape), renderer-free: fill 20×20 r6, border width 1.5 → the inside
//! path is 18.5×18.5 r5.25 at offset (0.75, 0.75); the outside path
//! 21.5×21.5 r6.75 at (−0.75, −0.75); center 20×20 r6 at origin.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tur_engine::core::layout::{Geometry, Offset, Size};
use tur_engine::core::render::brush::Color;
use tur_engine::core::render::{CanvasOp, RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

const SOURCE: &str = r#"
use tur_kit::flags::{ BorderPosition };
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ sv };

entry fn start() {
    let inside = Container().width_height(20.0, 20.0).radius(sv(6.0f64))
        .color(sv(0x111111FFu64))
        .border(0x0000FFFFu64, 1.5, BorderPosition::Inside)
        .build();
    let outside = Container().width_height(20.0, 20.0).radius(sv(6.0f64))
        .color(sv(0x111111FFu64))
        .border(0x00FF00FFu64, 1.5, BorderPosition::Outside)
        .build();
    let center = Container().width_height(20.0, 20.0).radius(sv(6.0f64))
        .color(sv(0x111111FFu64))
        .border(0x800080FFu64, 1.5, BorderPosition::Center)
        .build();
    mount(Column().child(inside).child(outside).child(center).build());
}
"#;

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

/// The one rounded-rect stroke carrying `color`.
fn stroke_of(ops: &[CanvasOp], color: Color) -> Option<(Offset, Size, f64, f64)> {
    ops.iter().find_map(|op| match op {
        CanvasOp::StrokeGeometry {
            offset,
            geometry: Geometry::RoundedRect { size, radius },
            color: c,
            stroke_width,
        } if *c == color => Some((*offset, *size, *radius, *stroke_width)),
        _ => None,
    })
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 0.001,
        "{what}: got {actual}, expected {expected}"
    );
}

#[test]
fn inside_border_stroke_deflates_the_corner_radius() {
    let mut app = TurTestApp::new(100.0, 100.0).unwrap();
    app.load_rut_module(SOURCE).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let ops = Rc::new(RefCell::new(Vec::new()));
    app.attach_renderer(
        Box::new(OpsRenderer { ops: ops.clone() }),
        100,
        100,
        1.0,
    );
    app.wait_for_timeout(Duration::ZERO);

    let ops = ops.borrow();
    let blue = Color::rgba(0x00, 0x00, 0xFF, 0xFF);
    let green = Color::rgba(0x00, 0xFF, 0x00, 0xFF);
    let purple = Color::rgba(0x80, 0x00, 0x80, 0xFF);

    // Inside: path 18.5×18.5 r5.25 at (0.75, 0.75) — the stroke's outer
    // edge lands exactly on the fill's r6 boundary.
    let (off, size, radius, width) = stroke_of(&ops, blue)
        .expect("the inside border stroke must paint");
    assert_close(off.x, 0.75, "inside offset.x");
    assert_close(off.y, 0.75, "inside offset.y");
    assert_close(size.width, 18.5, "inside size.width");
    assert_close(size.height, 18.5, "inside size.height");
    assert_close(radius, 6.0 - 1.5 / 2.0, "inside corner radius (deflate by bw/2)");
    assert_close(width, 1.5, "inside stroke width");

    // Outside: path 21.5×21.5 r6.75 at (−0.75, −0.75).
    let (off, size, radius, width) = stroke_of(&ops, green)
        .expect("the outside border stroke must paint");
    assert_close(off.x, -0.75, "outside offset.x");
    assert_close(off.y, -0.75, "outside offset.y");
    assert_close(size.width, 21.5, "outside size.width");
    assert_close(size.height, 21.5, "outside size.height");
    assert_close(radius, 6.0 + 1.5 / 2.0, "outside corner radius (inflate by bw/2)");
    assert_close(width, 1.5, "outside stroke width");

    // Center: straddles the boundary as-is — 20×20 r6 at the origin.
    let (off, size, radius, width) = stroke_of(&ops, purple)
        .expect("the center border stroke must paint");
    assert_close(off.x, 0.0, "center offset.x");
    assert_close(off.y, 0.0, "center offset.y");
    assert_close(size.width, 20.0, "center size.width");
    assert_close(size.height, 20.0, "center size.height");
    assert_close(radius, 6.0, "center corner radius (unchanged)");
    assert_close(width, 1.5, "center stroke width");
}
