//! Integration tests for the async clipboard + HTTP bridges.
//!
//! Validates the full spawn → tick → complete → drain → PromiseJob →
//! reactive-set path end-to-end:
//!
//! 1. JS calls `clipboard.readText()` / `request()` (ctx-bound fn pointers
//!    registered by the tur-clipboard / tur-net plugins).
//! 2. The fn creates a pending `JsPromise`, spawns a future via the
//!    engine's `AsyncExecutor` that calls `Clipboard::read_text().await`
//!    (or `Http::request(opts).await`).
//! 3. `flush`'s `tick` polls the future (Recording* impls resolve eagerly),
//!    the future pushes a `Completion` that resolves the promise.
//! 4. `drain_completions` runs the completion under `&mut Context`, which
//!    enqueues a `PromiseJob`.
//! 5. boa's `executor.drain` runs the PromiseJob → fires the `.then` body,
//!    which calls `set(source, ...)` → dirty → re-layout.
//! 6. Test asserts the source atom updated.
//!
//! Capability lookup: both bridge fns read their `Rc<dyn Clipboard>` /
//! `Rc<dyn Http>` / `Rc<AsyncExecutor>` from `TurJsContext`'s capability
//! registry (populated by the plugins during `register`). No `unsafe`
//! closures are involved.

use tur_integration_tests::{TurTestApp, text_response};

#[test]
fn clipboard_read_resolves_and_drives_reactive_set() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    // Pre-canned clipboard content.
    app.set_clipboard_read("hello from clipboard");

    // Set up: create a source atom, kick off a read, chain `.then` to set
    // the source with the resolved text. `clipboard.readText` is exported
    // by `tur:clipboard` as a method on the `clipboard` object.
    app.eval_module_source(
        r#"
        import { source, set, get } from "tur:std";
        import { clipboard } from "tur:clipboard";
        globalThis.__sink$ = source("initial");
        clipboard.readText().then((text) => {
            set(globalThis.__sink$, text);
            // Stash the resolved value as a plain string global so eval_js
            // (which runs as a script, not a module) can read it without
            // needing a bare `import`.
            globalThis.__result_text = String(get(globalThis.__sink$));
        });
        "#,
    )
    .unwrap();

    // Wait for the async chain to resolve: poll the spawned clipboard future,
    // drain the completion, run the PromiseJob, fire the `.then`.
    app.wait_for(|a| a.eval_js("globalThis.__result_text") == "hello from clipboard");

    assert_eq!(
        app.eval_js("globalThis.__result_text"),
        "hello from clipboard"
    );
}

#[test]
fn clipboard_write_logs_to_recording() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { clipboard } from "tur:clipboard";
        clipboard.writeText("payload");
        "#,
    )
    .unwrap();
    app.settle();

    assert_eq!(app.take_clipboard_write().as_deref(), Some("payload"));
}

#[test]
fn http_request_resolves_with_canned_response() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_response(text_response(200, "body bytes"));

    app.eval_module_source(
        r#"
        import { source, set } from "tur:std";
        import { request } from "tur:net";
        globalThis.__status$ = source(0);
        globalThis.__body$ = source("");
        request({ url: "https://example.test/x", method: "GET" })
            .then((r) => {
                set(globalThis.__status$, r.status);
                set(globalThis.__body$, r.bodyText);
                // Stash resolved values as plain string globals so eval_js
                // can read them without imports.
                globalThis.__result_status = String(r.status);
                globalThis.__result_body = String(r.bodyText);
            })
            .catch((e) => {
                globalThis.__result_body = "err:" + e.message;
            });
        "#,
    )
    .unwrap();
    app.wait_for(|a| a.eval_js("globalThis.__result_status") == "200");

    assert_eq!(app.eval_js("globalThis.__result_status"), "200");
    assert_eq!(app.eval_js("globalThis.__result_body"), "body bytes");
    assert_eq!(
        app.last_http_request(),
        Some(tur_integration_tests::RecordedRequest {
            url: "https://example.test/x".to_string(),
            method: "GET".to_string(),
        })
    );
}
