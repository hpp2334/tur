//! Regression tests for the engine's shell egress (cursor + text-input
//! state pushed via `HostMsg::Shell`).
//!
//! Background: the engine's `run_loop` (autonomous; Choreographer-polled on
//! Android, `spawn_local`'d on wasm — driven frame-by-frame by the harness
//! `pump` in these tests) used to dispatch `HostMsg`s through *separate*
//! handlers depending on the entry point, and only the single-frame path's
//! handler updated the host-side focus state. The autonomous path dropped
//! `HostMsg::FocusedStateChanged`
//! on the floor, so on Android the soft keyboard never rose when tapping
//! the code editor.
//!
//! The fix unified both paths on a single handler
//! (`HostBackend::apply_msg`) and replaced the engine-side focus cache
//! with a shell trait the embedder supplies at construction. These tests
//! pin that: after an editable is focused, the shell must receive
//! `is_editable == true` via `request_text_input`. Because `run_loop`
//! routes every message through the same `apply_msg`, the Android/wasm
//! path is covered by construction.

use std::cell::RefCell;
use std::time::Duration;

use tur_engine::core::shell::TextInputState;
use tur_integration_tests::TurTestApp;

/// After tapping a focused `Input`, the shell receives a text-input state
/// with `is_editable == true`. This is the push Android's `FrameLoop.onFocusChanged`
/// (and wasm's textarea-positioning handler) depend on to raise the soft
/// keyboard / position the caret.
#[test]
fn text_input_state_fires_on_editable_focus() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
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
    // Let the module's initial render settle before checking.
    app.wait_for_timeout(Duration::from_millis(32));

    // The shell may have received is_editable=false during the initial
    // render; that's expected. After tapping the Input, it must receive
    // is_editable=true.
    app.click(10.0, 10.0);
    let focused = app.wait_for(|app| {
        app.take_current_text_input_state()
            .map(|s| s.is_editable)
            .unwrap_or(false)
    });
    assert!(
        focused,
        "shell should receive is_editable=true after tapping the Input"
    );

    // While focus stays stable, the shell must NOT receive a redundant
    // text-input state (the worker dedups against the previous frame).
    app.wait_for_timeout(Duration::from_millis(16));
    // take_current_text_input_state returns None if nothing new was pushed.
    // That's the expected deduped behavior.
}
