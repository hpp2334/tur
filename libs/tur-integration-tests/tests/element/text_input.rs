//! Regression tests for the engine's shell text-input egress
//! (`Shell::request_text_input`, delivered via construction-time
//! `TurAppBuilder::shell`).
//!
//! Background: the engine's autonomous loop (Choreographer-polled on
//! Android, `spawn_local`'d on wasm — driven frame-by-frame by the harness
//! `pump` in these tests) used to dispatch `HostMsg`s through *separate*
//! handlers depending on the entry point, and only the single-frame path's
//! handler updated the host-side text-input state. The autonomous path
//! dropped `HostMsg::Shell(ShellCommand::RequestTextInput)` on the floor,
//! so on Android the soft keyboard never rose when tapping the code
//! editor.
//!
//! The fix unified both paths on a single handler
//! (`HostBackend::apply_msg`) and replaced the engine-side cache with a
//! construction-time shell the embedder supplies. These tests pin that:
//! the initial (inactive) text-input state is observable from frame 1
//! (the construction-time guarantee — a shell installed after `build()`
//! could miss the worker's first push on platforms where `build() returns
//! before worker readiness), and after an editable is focused the shell
//! receives `is_editable == true` on the `pump` path. Because the loop
//! routes every message through the same `apply_msg`, the Android/wasm
//! path is covered by construction.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tur_engine::core::scheduler::VsyncSource;
use tur_engine::core::shell::{Cursor, Shell, TextInputState};
use tur_integration_tests::TurTestApp;

/// Shell that records every text-input state the engine pushed. Carries
/// the harness-supplied vsync source as its frame clock (the factory in
/// `new_with_shell` receives it).
struct CaptureTextInput {
    states: Arc<Mutex<Vec<TextInputState>>>,
    vsync: Option<Rc<dyn VsyncSource>>,
}

impl Shell for CaptureTextInput {
    fn set_cursor(&mut self, _cursor: Cursor) {}

    fn request_text_input(&mut self, state: TextInputState) {
        self.states.lock().unwrap().push(state);
    }

    fn take_vsync(&mut self) -> Option<Rc<dyn VsyncSource>> {
        self.vsync.take()
    }
}

/// After an editable is focused, the shell's `request_text_input` fires
/// with `is_editable == true`. This is the push Android's
/// `FrameLoop.onTextInputChanged` (and wasm's textarea-positioning shell)
/// depend on to raise the soft keyboard / position the caret.
#[test]
fn text_input_requests_fire_on_editable_focus() {
    let states: Arc<Mutex<Vec<TextInputState>>> = Arc::new(Mutex::new(Vec::new()));
    let mut app = TurTestApp::new_with_shell(200.0, 100.0, |vsync| {
        Box::new(CaptureTextInput {
            states: states.clone(),
            vsync: Some(vsync),
        })
    })
    .unwrap();
    app.eval_module_source(
        r#"const store = createStore();

        import { createStore, Input, mount } from "tur:std";
        mount(store, Input({
            text: "",
            width: 200,
            height: 44,
            queryKey: ["editor"],
        }));
        "#,
    )
    .unwrap();
    // Let the module's initial render settle.
    app.wait_for_timeout(std::time::Duration::from_millis(32));

    // The worker's first pump ships the initial (inactive) text-input
    // state (dedup caches start empty), and construction-time shell
    // installation guarantees we observed it — the frame-1 guarantee.
    assert!(
        !states
            .lock()
            .unwrap()
            .last()
            .map(|s| s.is_editable)
            .unwrap_or(false),
        "no editable should be focused before any tap"
    );

    // Tap inside the Input → the focus manager focuses it. `click` is
    // fire-and-forget, so wait until the shell reports an editable is
    // focused (the loop routes the worker's shell command through the
    // shared `apply_msg`, firing `request_text_input`).
    app.click(10.0, 10.0);
    let focused = app.wait_for(|_| {
        states
            .lock()
            .unwrap()
            .last()
            .map(|s| s.is_editable)
            .unwrap_or(false)
    });
    assert!(
        focused,
        "shell should receive an editable text-input state after tapping the Input"
    );

    // While the text-input state stays stable, a subsequent frame must NOT
    // re-fire the request (the worker dedups against the previous ship).
    let count_before = states.lock().unwrap().len();
    app.wait_for_timeout(std::time::Duration::from_millis(16));
    assert_eq!(
        states.lock().unwrap().len(),
        count_before,
        "shell should not receive another text-input state while it stays stable (deduped)"
    );
}
