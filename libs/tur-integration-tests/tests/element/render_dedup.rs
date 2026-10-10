//! Frame dedup at the render commit point: a batch whose content
//! fingerprint matches the last APPLIED frame skips `render_commands` +
//! `present` entirely — the presented surface already shows that content.
//!
//! Pins:
//! - two identical painted frames → the renderer sees exactly one batch,
//! - a content change → applied again,
//! - attach/detach resets the fingerprint (a fresh renderer has painted
//!   nothing → the next batch must apply),
//! - the fingerprint is content-sensitive (any op difference re-applies).

use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::core::render::{RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

/// Renderer that counts `render_commands` invocations and records the last
/// batch (command count) for assertions.
struct CountingRenderer {
    calls: Rc<RefCell<u64>>,
    last_len: Rc<RefCell<usize>>,
}

impl Renderer for CountingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        *self.calls.borrow_mut() += 1;
        *self.last_len.borrow_mut() = commands.len();
    }
}

/// CountingRenderer + a quiesced app with one mounted container: the initial
/// frame applies once; identical repaints are deduped away.
#[test]
fn identical_frames_render_once() {
    let calls = Rc::new(RefCell::new(0u64));
    let renderer = CountingRenderer {
        calls: calls.clone(),
        last_len: Rc::new(RefCell::new(0)),
    };
    let app = TurTestApp::new_with_renderer(300.0, 300.0, Box::new(renderer)).expect("app");
    app.load_rut_module(
        r#"

use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::reactive::{ MutationCtx, Readable, Source, source };

entry fn start() {
    let b = Container().width_height(100.0, 50.0).color(0xFF0000FFu64);
    mount(b.build());
}
"#,
    )
    .expect("mount");
    // Drive the initial paint through.
    app.wait_for_timeout(std::time::Duration::ZERO);
    let after_first = *calls.borrow();
    assert!(
        after_first >= 1,
        "initial paint must apply once, got {after_first}"
    );

    // Drive several more frames without changing anything — the worker
    // repaints on every pump (see the idle-repaint note), but identical
    // batches must be deduped at the commit point.
    for _ in 0..5 {
        app.pump();
    }
    assert_eq!(
        *calls.borrow(),
        after_first,
        "identical repaints must not re-apply"
    );
}

/// A content change re-applies: change the container's color → the next
/// batch differs → the renderer applies again.
#[test]
fn changed_content_reapplies() {
    let calls = Rc::new(RefCell::new(0u64));
    let renderer = CountingRenderer {
        calls: calls.clone(),
        last_len: Rc::new(RefCell::new(0)),
    };
    let app = TurTestApp::new_with_renderer(300.0, 300.0, Box::new(renderer)).expect("app");
    // Visible container + a brush atom so the test can flip it.
    app.load_rut_module(
        r#"
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::reactive::{ Source, entry_ctx, source };

struct AppContext {
    color: Source<u64>,
}

entry fn start() -> opaque {
    // The boot brush seeds at the mint.
    let color: Source<u64> = source<u64>(0xFF0000FFu64);

    let b = Container().width_height(100.0, 50.0).color_bound(color);
    mount(b.build());
    return opaque(AppContext { color: color });
}

entry fn do_set(cx: opaque, v: f64) {
    // 0 clears the brush (the decode refuses 0 — the prop resolves
    // absent); the container repaints unpainted (the batch differs
    // either way).
    let c = opaque.downcast<AppContext>(cx);
    if (c == nil) {
        panic("dedup fixture: cx is not an AppContext");
    }
    let write = entry_ctx();
    if (v == 0.0) {
        write.set<u64>(c.color, 0);
    } else {
        write.set<u64>(c.color, 0x00FF00FFu64);
    }
}
"#,
    )
    .expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);
    let cx = app.rut_start_answer();
    let after_initial = *calls.borrow();
    assert!(after_initial >= 1);

    // Flip the color: the batch differs → must apply.
    app.call_rut_entry_cx_f64("do_set", cx, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let after_flip = *calls.borrow();
    assert!(
        after_flip > after_initial,
        "changed content must re-apply ({after_initial} → {after_flip})"
    );

    // Identical afterwards: deduped again.
    for _ in 0..5 {
        app.pump();
    }
    assert_eq!(
        *calls.borrow(),
        after_flip,
        "frames identical to the flipped frame must not re-apply"
    );
}

/// Attach/detach resets the dedup signal: after a detach + attach, the next
/// batch applies even if identical to the last applied frame (the fresh
/// renderer has painted nothing).
#[test]
fn attach_resets_dedup() {
    let calls = Rc::new(RefCell::new(0u64));
    let renderer = CountingRenderer {
        calls: calls.clone(),
        last_len: Rc::new(RefCell::new(0)),
    };
    let app = TurTestApp::new_with_renderer(300.0, 300.0, Box::new(renderer)).expect("app");
    app.load_rut_module(
        r#"

use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::reactive::{ MutationCtx, Readable, Source, source };
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::reactive::{ MutationCtx, Readable, Source, source };

entry fn start() {
    let b = Container().width_height(100.0, 50.0).color(0xFF0000FFu64);
    mount(b.build());
}
"#,
    )
    .expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);
    let after_initial = *calls.borrow();
    assert!(after_initial >= 1);

    // Detach + attach the same-content renderer: the next identical batch
    // must apply (the new renderer painted nothing yet).
    app.detach_renderer();
    app.attach_renderer(
        Box::new(CountingRenderer {
            calls: calls.clone(),
            last_len: Rc::new(RefCell::new(0)),
        }),
        300,
        300,
        1.0,
    );
    app.pump();
    let after_attach = *calls.borrow();
    assert!(
        after_attach > after_initial,
        "freshly attached renderer must apply the identical batch ({after_initial} → {after_attach})"
    );
}
