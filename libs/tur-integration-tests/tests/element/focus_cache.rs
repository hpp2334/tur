//! Regression tests for the engine's focus-change handler
//! (`TurApp::set_focus_changed_handler`).
//!
//! Background: the engine has two execution paths over the worker→main
//! channel — `TurApp::pump` (single-frame; used by these tests) and
//! `TurApp::run_loop` (autonomous; Choreographer-polled on Android,
//! `spawn_local`'d on wasm). Both used to dispatch `MainMsg`s through
//! *separate* handlers, and only `pump`'s handler updated the main-side
//! focus state. The autonomous path dropped `MainMsg::FocusedStateChanged`
//! on the floor, so on Android the soft keyboard never rose when tapping
//! the code editor.
//!
//! The fix unified both paths on a single handler
//! (`MainBackend::apply_msg`) and replaced the engine-side focus cache
//! with a push handler the embedder registers. These tests pin that: after
//! an editable is focused, the handler must fire with `is_editable == true`
//! on the `pump` path. Because `run_loop` routes every message through the
//! same `apply_msg`, the Android/wasm path is covered by construction.

use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::FocusedState;
use tur_integration_tests::TurTestApp;

/// After tapping a focused `Input`, the focus-change handler fires with an
/// editable focused. This is the push Android's `FrameLoop.onFocusChanged`
/// (and wasm's textarea-positioning handler) depend on to raise the soft
/// keyboard / position the caret.
#[test]
fn focus_changed_handler_fires_on_editable_focus() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
        import { Input, render } from "tur:std";
        render(Input({
            text: "",
            width: 200,
            height: 44,
            queryKey: ["editor"],
        }));
        "#,
    )
    .unwrap();

    // Capture the latest focus state the handler observed. `None` = not yet.
    let captured: Rc<RefCell<Option<FocusedState>>> = Rc::new(RefCell::new(None));
    {
        let captured = captured.clone();
        app.app()
            .set_focus_changed_handler(Some(Rc::new(move |state| {
                *captured.borrow_mut() = Some(state);
            })));
    }

    // Nothing focused yet (the handler may have fired during the eval pump
    // with is_editable=false; either way there's no editable focused).
    assert!(
        !captured
            .borrow()
            .as_ref()
            .map(|s| s.is_editable)
            .unwrap_or(false),
        "no editable should be focused before any tap"
    );

    // Tap inside the Input → the focus manager focuses it. `click` flushes
    // a frame, so `apply_msg` has fired the handler with the worker's
    // `MainMsg::FocusedStateChanged` by the time it returns.
    app.click(10.0, 10.0);

    assert!(
        captured
            .borrow()
            .as_ref()
            .map(|s| s.is_editable)
            .unwrap_or(false),
        "focus-change handler should report an editable is focused after tapping the Input"
    );

    // A subsequent pump while focus is stable must NOT re-fire the handler
    // (the worker dedups FocusedStateChanged against the previous frame).
    let before = captured.borrow().clone();
    app.pump().unwrap();
    assert_eq!(
        *captured.borrow(),
        before,
        "handler should not re-fire while focus stays stable (deduped)"
    );
}
