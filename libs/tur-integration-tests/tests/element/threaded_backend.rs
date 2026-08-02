//! Phase 7/8 smoke tests — prove `ThreadedBackend` dispatches across the
//! thread boundary, both via low-level `ThreadedBackend::new(factory)` and
//! via the high-level `TurRuntime::create_app_threaded(...)` runtime API.
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

    // RPC #1: load_module — Reply round-trips across the thread boundary
    // via the Condvar.
    app.load_module(
        "import { Column, Text } from 'tur:std';\
         globalThis.__root = () => Column({ children: [ Text('hi') ] });",
    )
    .expect("load_module round-tripped across threads");

    // RPC #2: pump — sends Wake, worker runs flush, ships FrameOutcome.
    let outcome = app.pump().expect("pump round-tripped across threads");
    eprintln!("threaded pump outcome: rendered={}", outcome.rendered);

    // RPC #3: dev-tool query (separate Reply-slot round-trip).
    let _tree = app.dev_tool_element_tree();

    // RPC #4: eval_module (another Reply-slot path).
    app.eval_module("export const x = 1;")
        .expect("eval_module round-tripped across threads");

    // RPC #5: focused-state queries (wasm hot path — IME / caret).
    let _state = app.focused_state();
    let _editable = app.focused_is_editable();
    let _rect = app.focused_cursor_rect();
    let _id = app.focused_element();

    // RPC #6: push_app_event (fire-and-forget).
    app.request_paint();
    app.pump().expect("pump after request_paint");

    // RPC #7: render_to_pixels (returns None for NoopRenderer).
    let _pixels = app.render_to_pixels();

    // RPC #8: event_bus_handle — emit_to_js ships via WorkerMsg::EventBusToJs.
    let bus = app.event_bus_handle();
    bus.emit_to_js(b"hello from threaded main".to_vec());
    // Worker's subsystem will deliver on next flush.
    app.pump().expect("pump after event_bus emit_to_js");

    // RPC #9: set_cursor_backend — stored on main. Worker emits
    // MainMsg::CursorChanged on cursor change; main applies here.
    use std::cell::RefCell;
    use std::rc::Rc;
    #[derive(Default)]
    struct RecordingCursor(std::cell::RefCell<Option<tur_engine::core::platform::Cursor>>);
    impl tur_engine::core::platform::CursorBackend for RecordingCursor {
        fn set_cursor(&mut self, cursor: tur_engine::core::platform::Cursor) {
            *self.0.borrow_mut() = Some(cursor);
        }
    }
    let recording = Rc::new(RefCell::new(RecordingCursor::default()));
    app.set_cursor_backend(recording.clone() as Rc<RefCell<dyn tur_engine::core::platform::CursorBackend>>);
    // No pump here — without a pointer event, the worker's cursor sink
    // stays empty. The point is that `set_cursor_backend` doesn't panic.
}

#[test]
fn runtime_create_app_threaded_end_to_end() {
    use tur_engine::TurRuntime;

    // Build runtime with TurStdPlugin — the threaded factory
    // re-registers it on the worker.
    let runtime = TurRuntime::builder()
        .font_loader(std::sync::Arc::new(StubFontLoader))
        .clock(std::sync::Arc::new(StdClock::new()))
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    // create_app_threaded spawns the worker, captures Arc clones of
    // clock/font_loader/plugins, constructs InlineBackend on the worker,
    // wraps in ThreadedBackend.
    let app = runtime
        .create_app_threaded(
            || Box::new(tur_engine::renderer::NoopRenderer::new()),
            (200.0, 100.0),
            1.0,
        )
        .expect("create_app_threaded");

    // Verify the canonical embedder flow: load_module → pump →
    // focused_state (the wasm website hot path).
    app.load_module("export const x = 1;")
        .expect("load_module via runtime-spawned worker");
    let outcome = app.pump().expect("pump via runtime-spawned worker");
    let _state = app.focused_state();
    eprintln!(
        "runtime.create_app_threaded end-to-end: rendered={}",
        outcome.rendered
    );
}
