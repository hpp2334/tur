//! Integration tests for the `Task<T> = { promise, cancel() }` async model:
//! every async bridge API (`sleep`, `clipboard.*`, `request`,
//! `requestStream`, `filePicker.*`) returns a Task handle — never a bare
//! Promise, never a generator.
//!
//! Pins:
//! 1. Shape: `{ promise: Promise<T>, cancel(): void }`.
//! 2. `promise` settles with the op's result via `.then` / `await`.
//! 3. `cancel()` **rejects the promise with a `CancelError`**
//!    (`e.name === "CancelError"`, `isCancelError(e) === true`); idempotent;
//!    no-op after settlement.
//! 4. Cancellation really stops work where stoppable: a cancelled `sleep`
//!    never fires; a cancelled `request`/stream aborts the driver (no/less
//!    recorded I/O); a mid-stream cancel ends iteration with
//!    `{done: true}` (so `for await` exits cleanly).

use tur_filepicker_capability::PickedFile;
use tur_integration_tests::{TurTestApp, text_response};
use tur_net_capability::HttpOutcome;

// ── shape + sleep ──────────────────────────────────────────────────────────

#[test]
fn sleep_returns_task_shape() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        const t = sleep(10);
        globalThis.__hasPromise = String(typeof t.promise);
        globalThis.__thenable = String(typeof t.promise.then);
        globalThis.__hasCancel = String(typeof t.cancel);
        "#,
    )
    .unwrap();

    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(app.eval_js("globalThis.__hasPromise"), "object");
    assert_eq!(app.eval_js("globalThis.__thenable"), "function");
    assert_eq!(app.eval_js("globalThis.__hasCancel"), "function");
}

#[test]
fn sleep_promise_resolves_after_delay() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        globalThis.__done = "0";
        sleep(50).promise.then(() => { globalThis.__done = "1"; });
        "#,
    )
    .unwrap();

    // Not resolved synchronously / after a single frame.
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(app.eval_js("globalThis.__done"), "0");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "1");
    assert_eq!(app.eval_js("globalThis.__done"), "1");
}

#[test]
fn async_await_works_end_to_end() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        globalThis.__v = "0";
        (async () => {
            globalThis.__v = "1";
            await sleep(30).promise;
            globalThis.__v = "2";
        })();
        "#,
    )
    .unwrap();

    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(app.eval_js("globalThis.__v"), "1");
    app.wait_for(|a| a.eval_js("globalThis.__v") == "2");
    assert_eq!(app.eval_js("globalThis.__v"), "2");
}

/// `cancel()` rejects with a `CancelError`; `isCancelError` recognizes it.
#[test]
fn cancel_rejects_with_cancel_error() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { isCancelError, sleep } from "tur:std";
        globalThis.__state = "pending";
        const t = sleep(5000);
        t.promise.then(
            () => { globalThis.__state = "resolved"; },
            (e) => {
                globalThis.__state = "rejected";
                globalThis.__name = String(e.name);
                globalThis.__isCancel = String(isCancelError(e));
            },
        );
        t.cancel();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__state") == "rejected");
    assert_eq!(app.eval_js("globalThis.__name"), "CancelError");
    assert_eq!(app.eval_js("globalThis.__isCancel"), "true");
}

/// A cancelled sleep never fires its fulfillment handler — and it does not
/// fire even after the original deadline passes (the timer is really
/// aborted, not ignored).
#[test]
fn cancelled_sleep_never_fires_even_after_deadline() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        globalThis.__fired = "no";
        const t = sleep(60);
        t.promise.then(() => { globalThis.__fired = "yes"; }, () => {});
        t.cancel();
        "#,
    )
    .unwrap();

    app.wait_for_timeout(std::time::Duration::from_millis(200));
    assert_eq!(app.eval_js("globalThis.__fired"), "no");
}

/// Double cancel → a single rejection; cancel after resolution is a no-op
/// (the promise stays resolved).
#[test]
fn cancel_is_idempotent_and_post_completion_noop() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        globalThis.__rejections = 0;
        const a = sleep(30);
        a.promise.then(() => { globalThis.__a = "resolved"; },
                       () => { globalThis.__rejections += 1; });
        a.cancel();
        a.cancel();

        const b = sleep(20);
        b.promise.then(() => {
            globalThis.__b = "resolved";
            b.cancel();
        }, () => { globalThis.__b = "rejected"; });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__b") == "resolved");
    app.wait_for_timeout(std::time::Duration::from_millis(80));
    assert_eq!(app.eval_js("globalThis.__rejections"), "1");
    assert_eq!(app.eval_js("globalThis.__a"), "undefined");
    assert_eq!(app.eval_js("globalThis.__b"), "resolved");
}

/// The debounce idiom: cancel the previous delay; only the latest fires.
#[test]
fn debounce_pattern_only_latest_fires() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        let t = null;
        function schedule(tag) {
            t?.cancel();
            t = sleep(40);
            t.promise.then(() => { globalThis.__fired = tag; }, () => {});
        }
        schedule("first");
        schedule("second");
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__fired") == "second");
    app.wait_for_timeout(std::time::Duration::from_millis(120));
    assert_eq!(app.eval_js("globalThis.__fired"), "second");
}

// ── tur:net ────────────────────────────────────────────────────────────────

#[test]
fn request_returns_task_and_resolves() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_response(text_response(200, "task body"));

    app.eval_module_source(
        r#"
        import { request } from "tur:net";
        globalThis.__done = "no";
        const t = request({ url: "https://example.test/x", method: "GET" });
        t.promise.then((r) => {
            globalThis.__status = String(r.status);
            globalThis.__body = String(r.bodyText);
            globalThis.__done = "yes";
        });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__done") == "yes");
    assert_eq!(app.eval_js("globalThis.__status"), "200");
    assert_eq!(app.eval_js("globalThis.__body"), "task body");
}

/// Cancelling before the driver is polled aborts it — the request is never
/// sent (`last_http_request` stays `None`) and the promise rejects
/// `CancelError`.
#[test]
fn request_cancel_aborts_before_send() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_response(text_response(200, "never read"));

    app.eval_module_source(
        r#"
        import { request } from "tur:net";
        globalThis.__state = "pending";
        const t = request({ url: "https://example.test/aborted" });
        t.promise.then(
            () => { globalThis.__state = "resolved"; },
            (e) => { globalThis.__state = String(e.name); },
        );
        t.cancel();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__state") == "CancelError");
    app.wait_for_timeout(std::time::Duration::from_millis(50));
    assert_eq!(
        app.last_http_request(),
        None,
        "cancelled request never sent"
    );
}

#[test]
fn stream_cancel_before_response_rejects_cancel_error() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_stream(200, vec![b"chunk-0".to_vec(), b"chunk-1".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        globalThis.__state = "pending";
        const t = requestStream({ url: "http://test/never" });
        t.promise.then(
            () => { globalThis.__state = "resolved"; },
            (e) => { globalThis.__state = String(e.name); },
        );
        t.cancel();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__state") == "CancelError");
}

/// Cancelling mid-consumption: the pending pull (and every later one)
/// resolves `{done: true}` so `for await` / manual loops exit cleanly, and
/// the producer is no longer pulled (wire abort).
#[test]
fn stream_cancel_mid_consumption_ends_iteration() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_stream(
        200,
        vec![
            b"chunk-0!!".to_vec(),
            b"chunk-1!!".to_vec(),
            b"chunk-2!!".to_vec(),
            b"chunk-3!!".to_vec(),
        ],
    );

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        globalThis.__done = "no";
        (async () => {
            const t = requestStream({ url: "http://test/mid" });
            globalThis.__t = t;
            const resp = await t.promise;
            let r = await resp.body.next();
            globalThis.__first = r.done ? "" : String(Array.from(r.value).length);
            const pending = resp.body.next();     // second pull in flight
            globalThis.__t.cancel();               // abort mid-stream
            r = await pending;
            globalThis.__pendingDone = String(r.done);
            r = await resp.body.next();            // subsequent → done immediately
            globalThis.__afterDone = String(r.done);
            globalThis.__done = "yes";
        })();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__done") == "yes");
    assert_eq!(app.eval_js("globalThis.__first"), "9");
    assert_eq!(app.eval_js("globalThis.__pendingDone"), "true");
    assert_eq!(app.eval_js("globalThis.__afterDone"), "true");
    assert_eq!(
        app.http_stream_pulls(),
        2,
        "two pulls before cancel; the abort stops the producer"
    );
}

// ── tur:clipboard ──────────────────────────────────────────────────────────

#[test]
fn clipboard_read_write_via_task_promise() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.set_clipboard_read("hello task");

    app.eval_module_source(
        r#"
        import { clipboard } from "tur:clipboard";
        globalThis.__done = "no";
        clipboard.readText().promise.then((text) => {
            globalThis.__text = String(text);
            return clipboard.writeText("task payload").promise;
        }).then(() => { globalThis.__done = "yes"; });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__done") == "yes");
    assert_eq!(app.eval_js("globalThis.__text"), "hello task");
    assert_eq!(app.take_clipboard_write().as_deref(), Some("task payload"));
}

// ── tur:filepicker ─────────────────────────────────────────────────────────

#[test]
fn pick_and_save_via_task_promise() {
    let mut app = TurTestApp::new_with_filepicker(200.0, 100.0).unwrap();
    app.set_next_pick(vec![PickedFile {
        name: "a.txt".to_string(),
        bytes: b"hello".to_vec(),
        mime_type: Some("text/plain".to_string()),
    }]);

    app.eval_module_source(
        r#"
        import { filePicker } from "tur:filepicker";
        globalThis.__done = "no";
        (async () => {
            const files = await filePicker.pick().promise;
            globalThis.__count = String(files.length);
            globalThis.__name = String(files[0] && files[0].name);
            const bytes = new ArrayBuffer(2);
            new Uint8Array(bytes)[0] = 1;
            new Uint8Array(bytes)[1] = 2;
            await filePicker.saveFile("out.bin", bytes).promise;
            globalThis.__done = "yes";
        })();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__done") == "yes");
    assert_eq!(app.eval_js("globalThis.__count"), "1");
    assert_eq!(app.eval_js("globalThis.__name"), "a.txt");
    let saved = app.last_save().expect("save recorded");
    assert_eq!(saved.name, "out.bin");
    assert_eq!(saved.bytes, vec![1, 2]);
}

// The `HttpOutcome` import is used by the net tests above via text_response;
// keep the explicit path referenced so the import stays meaningful.
#[test]
fn request_error_still_rejects_with_message() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_response(HttpOutcome::Err("network down".to_string()));

    app.eval_module_source(
        r#"
        import { request } from "tur:net";
        globalThis.__done = "no";
        request({ url: "https://example.test/e" }).promise.then(
            () => { globalThis.__caught = "unreachable"; },
            (e) => { globalThis.__caught = String(e.message); globalThis.__done = "yes"; },
        );
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__done") == "yes");
    assert_eq!(app.eval_js("globalThis.__caught"), "network down");
}
