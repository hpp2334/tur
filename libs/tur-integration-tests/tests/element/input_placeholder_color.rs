//! Default placeholder color of `Input` (unfocused + empty controller): the
//! element's text `color` (or the default text color when unset) mixed with
//! 50% alpha — CSS `color-mix(in srgb, currentColor 50%, transparent)`
//! semantics: channels preserved, alpha multiplied by 0.5. An explicit
//! `placeholderColor` still wins outright.
//!
//! The tests capture the frame's `RenderCommand` batch with a recording
//! renderer and read the placeholder run's brush straight out of the
//! `FillTextLayout` op — font-independent ground truth (no pixel sampling).

use std::cell::RefCell;
use std::rc::Rc;

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

/// Brush of every text run painted this frame (across all `FillTextLayout`s).
fn text_run_brushes(cmds: &[RenderCommand]) -> Vec<[u8; 4]> {
    let mut brushes = Vec::new();
    for cmd in cmds {
        let RenderCommand::Paint { ops, .. } = cmd;
        for op in ops {
            if let CanvasOp::FillTextLayout { layout, .. } = op {
                brushes.extend(layout.runs.iter().map(|r| r.brush));
            }
        }
    }
    brushes
}

fn mount(app: &mut TurTestApp, source: &str) {
    app.load_rut_module(source).expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);
}

/// Without `color` / `placeholderColor`, the placeholder paints as the
/// default text color (opaque black) at 50% alpha — not the old fixed gray.
#[test]
fn placeholder_default_is_default_text_color_mixed_50pct_alpha() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        300.0,
        60.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    mount(
        &mut app,
        r#"
use tur_host::{ mount };
use tur_kit::{ Input };


entry fn start() {
    let input = Input().placeholder("hint").font_size(20.0).width_height(300.0, 40.0);
    mount(input.build());
}
"#,
    );

    let brushes = text_run_brushes(&last.borrow());
    assert_eq!(
        brushes,
        vec![[0, 0, 0, 128]],
        "default placeholder must be the default text color (black) mixed with 50% alpha"
    );
}

/// The default follows an explicit `color` (currentColor semantics): channels
/// preserved, alpha multiplied by 0.5.
#[test]
fn placeholder_default_follows_explicit_text_color() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        300.0,
        60.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    mount(
        &mut app,
        r#"
use tur_host::{ mount };
use tur_kit::{ Input };



entry fn start() {
    let input = Input()
        .placeholder("hint")
        .color(0x143CDCFFu64)
        .font_size(20.0)
        .width_height(300.0, 40.0);
    mount(input.build());
}
"#,
    );

    let brushes = text_run_brushes(&last.borrow());
    assert_eq!(
        brushes,
        vec![[20, 60, 220, 128]],
        "default placeholder must mix the explicit text color with 50% alpha"
    );
}

/// A `color` that already carries alpha composes: the mix multiplies the
/// existing alpha by 0.5 (color-mix with transparent), it does not reset it.
#[test]
fn placeholder_default_multiplies_existing_alpha() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        300.0,
        60.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    mount(
        &mut app,
        r#"
use tur_host::{ mount };
use tur_kit::{ Input };



entry fn start() {
    let input = Input()
        .placeholder("hint")
        .color(0x000000C8u64)
        .font_size(20.0)
        .width_height(300.0, 40.0);
    mount(input.build());
}
"#,
    );

    let brushes = text_run_brushes(&last.borrow());
    assert_eq!(
        brushes,
        vec![[0, 0, 0, 100]],
        "default placeholder must multiply an existing text alpha by 0.5 (200 → 100)"
    );
}

/// An explicit `placeholderColor` still wins — the mix is only the DEFAULT.
#[test]
fn explicit_placeholder_color_wins() {
    let last: Rc<RefCell<Vec<RenderCommand>>> = Rc::new(RefCell::new(Vec::new()));
    let mut app = TurTestApp::new_with_renderer(
        300.0,
        60.0,
        Box::new(RecordingRenderer { last: last.clone() }),
    )
    .expect("app");

    mount(
        &mut app,
        r#"
use tur_host::{ mount };
use tur_kit::{ Input };

entry fn start() {
    let input = Input()
        .placeholder("hint")
        .color(0x143CDCFFu64)
        .placeholder_color(0x008000FFu64)
        .font_size(20.0)
        .width_height(300.0, 40.0);
    mount(input.build());
}
"#,
    );

    let brushes = text_run_brushes(&last.borrow());
    assert_eq!(
        brushes,
        vec![[0, 128, 0, 255]],
        "an explicit placeholderColor must be used verbatim"
    );
}
