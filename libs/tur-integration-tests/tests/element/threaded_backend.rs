//! Phase 7 smoke test — proves `ThreadedBackend` dispatches across the
//! thread boundary. Uses `build_inline_backend` inside a Send factory
//! closure that constructs all engine pieces on the worker thread.
use std::sync::Arc;

use boa_engine::context::time::StdClock;

use tur_engine::core::fonts::FontLoader;
use tur_engine::core::runtime::{ThreadedBackend, build_inline_backend};
use tur_engine::{TurApp, TurStdPlugin};

struct StubFontLoader;
impl FontLoader for StubFontLoader {
    fn load_preset_fonts(&self, _: &mut tur_engine::core::fonts::FontContext) {}
}

/// Build an `InlineBackend` on the calling thread. Used inside the
/// threaded factory closure — runs ON THE WORKER THREAD.
fn build_backend() -> tur_engine::core::runtime::InlineBackend {
    let plugins: Vec<Box<dyn tur_engine::core::plugin::Plugin>> = vec![Box::new(TurStdPlugin)];

    build_inline_backend(
        Arc::new(StdClock::new()),
        {
            let mut fc = tur_engine::core::fonts::FontContext::new();
            StubFontLoader.load_preset_fonts(&mut fc);
            fc
        },
        Arc::new(StubFontLoader),
        tur_engine::core::capability::Capabilities::new(),
        &plugins,
        Box::new(tur_engine::renderer::NoopRenderer::new()),
        (200.0, 100.0),
    )
    .expect("build_inline_backend")
}

#[test]
fn threaded_app_cross_thread_rpc() {
    let app = std::rc::Rc::new(TurApp::new(Box::new(ThreadedBackend::new(build_backend))));

    // RPC #1: load a module. Reply round-trips across the thread
    // boundary via the Condvar. Verifies:
    //   - worker thread spawned correctly
    //   - mpsc channel send/recv works
    //   - Reply slot Condvar wakes main
    //   - InlineBackend.handle_worker_msg(LoadModule) processed on worker
    //   - reply.send(Result<(), ModuleError>) delivered to main
    app.load_module(
        "import { Column, Text } from 'tur:std';\
         globalThis.__root = () => Column({ children: [ Text('hi') ] });",
    )
    .expect("load_module round-tripped across threads");

    // RPC #2: pump. Sends WorkerMsg::Wake, worker runs flush, ships
    // MainMsg::FrameOutcome, main drains main_rx.
    let outcome = app.pump().expect("pump round-tripped across threads");
    eprintln!("threaded pump outcome: rendered={}", outcome.rendered);

    // RPC #3: dev-tool query (separate Reply-slot round-trip).
    // Returns Option<DevNodeData>. None is OK — the JS above sets
    // globalThis.__root but doesn't actually mount a tree, so the
    // engine has nothing to render. The point is that the RPC itself
    // works (returns without hanging / erroring).
    let _tree = app.dev_tool_element_tree();
    eprintln!("threaded dev_tool_element_tree RPC completed");

    // RPC #4: eval_module (another Reply-slot path).
    app.eval_module("export const x = 1;")
        .expect("eval_module round-tripped across threads");

    // RPC #5: focused-state queries (the embedder hot path — wasm reads
    // these every frame for IME / caret placement).
    let _state = app.focused_state();
    let _editable = app.focused_is_editable();
    let _rect = app.focused_cursor_rect();
    let _id = app.focused_element();
    eprintln!("threaded focused-state queries completed");

    // RPC #6: push_app_event (fire-and-forget across threads). We just
    // verify it doesn't panic / hang.
    // Using a no-op event since we don't have a way to verify delivery
    // without more plumbing — the round-trip itself is the test.
    app.request_paint();
    app.pump().expect("pump after request_paint");

    // RPC #7: render_to_pixels (returns None for NoopRenderer).
    let pixels = app.render_to_pixels();
    eprintln!("threaded render_to_pixels: {:?}", pixels.is_some());
}
