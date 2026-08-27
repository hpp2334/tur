//! Integration tests for `tur:net.requestStream` — the generator-based
//! streaming HTTP request (`yield*` from a `launch` coroutine) with an
//! async-iterable response body.
//!
//! Validates the full path: JS `yield* requestStream(opts)` → `RecordingHttp`
//! serves canned chunks → the async iterable yields `Uint8Array` chunks →
//! JS collects and concatenates them. Plus the backpressure surface:
//! pull-only-what's-consumed, `bufferBytes` validation, `cancel()`, and the
//! serial-`next()` protocol.

use tur_integration_tests::TurTestApp;

#[test]
fn stream_collects_multiple_chunks() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    // Serve three chunks that concatenate to "Hello, streaming world!"
    app.set_http_stream(
        200,
        vec![
            b"Hello, ".to_vec(),
            b"streaming ".to_vec(),
            b"world!".to_vec(),
        ],
    );

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__collected = "";
        globalThis.__done = false;

        launch(function*() {
            const resp = yield* requestStream({ url: "http://test/stream" });
            globalThis.__status = resp.status;

            let result = yield resp.body.next();
            while (!result.done) {
                const text = Array.from(result.value)
                    .map(b => String.fromCharCode(b))
                    .join("");
                globalThis.__collected += text;
                result = yield resp.body.next();
            }
            globalThis.__done = true;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__status"), "200");
    assert_eq!(
        app.eval_js("globalThis.__collected"),
        "Hello, streaming world!"
    );
}

#[test]
fn stream_single_chunk() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(200, vec![b"one chunk".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__collected = "";
        globalThis.__done = false;

        launch(function*() {
            const resp = yield* requestStream({ url: "http://test/single" });

            let result = yield resp.body.next();
            while (!result.done) {
                const text = Array.from(result.value)
                    .map(b => String.fromCharCode(b))
                    .join("");
                globalThis.__collected += text;
                result = yield resp.body.next();
            }
            globalThis.__done = true;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__collected"), "one chunk");
}

#[test]
fn stream_empty_body() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    // No chunks at all — body should immediately return {done: true}
    app.set_http_stream(204, vec![]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__done = false;

        launch(function*() {
            const resp = yield* requestStream({ url: "http://test/empty" });
            let result = yield resp.body.next();
            globalThis.__done = result.done;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");
}

/// Backpressure pin at the bridge seam: the body stream is pulled exactly
/// once per JS `next()` call — no prefetch. Two consumed chunks ⇒ exactly two
/// produced, three remain unproduced.
#[test]
fn stream_pulls_only_what_js_consumes() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(
        200,
        vec![
            b"chunk-0!!".to_vec(),
            b"chunk-1!!".to_vec(),
            b"chunk-2!!".to_vec(),
            b"chunk-3!!".to_vec(),
            b"chunk-4!!".to_vec(),
        ],
    );

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__done = false;

        launch(function*() {
            const resp = yield* requestStream({ url: "http://test/pull" });
            let r = yield resp.body.next();
            globalThis.__first = r.done ? "" : Array.from(r.value).length;
            r = yield resp.body.next();
            globalThis.__second = !r.done;
            globalThis.__done = true;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__first"), "9");
    assert_eq!(app.eval_js("globalThis.__second"), "true");
    assert_eq!(
        app.http_stream_pulls(),
        2,
        "the bridge must poll exactly one chunk per next() — no prefetch"
    );
}

/// `bufferBytes` is validated at parse time: bad values throw at the
/// `yield*` with a clear message (catchable like any generator error — the
/// composable-try/catch ergonomics the generator API exists for); a valid
/// value passes through to the backend.
#[test]
fn buffer_bytes_validation() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();
    app.set_http_stream(200, vec![b"ok".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__done = false;
        globalThis.__msgs = [];

        // A generator helper around yield* — retries/timeouts compose the
        // same way, with no promise chains.
        function* attempt(label, opts) {
            try {
                yield* requestStream(opts);
                globalThis.__msgs.push(label + ":ok");
            } catch (e) {
                globalThis.__msgs.push(label + ":" + e.message);
            }
        }

        launch(function*() {
            yield* attempt("zero", { url: "http://test/v", bufferBytes: 0 });
            yield* attempt("frac", { url: "http://test/v", bufferBytes: 1.5 });
            yield* attempt("huge", { url: "http://test/v", bufferBytes: 1e9 });
            yield* attempt("str", { url: "http://test/v", bufferBytes: "1024" });
            yield* attempt("valid", { url: "http://test/v", bufferBytes: 1024 });
            globalThis.__done = true;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    let msgs = app.eval_js("globalThis.__msgs.join(\"|\")");
    assert!(msgs.contains("zero:bufferBytes must be >= 1"), "{msgs}");
    assert!(msgs.contains("frac:bufferBytes must be an integer"), "{msgs}");
    assert!(
        msgs.contains("huge:bufferBytes must be <= 67108864 (64 MiB)"),
        "{msgs}"
    );
    assert!(msgs.contains("str:bufferBytes must be a number"), "{msgs}");
    assert!(msgs.contains("valid:ok"), "{msgs}");
}

/// `body.cancel()` aborts deterministically: pending/subsequent `next()`
/// resolve `{done: true}` and the remaining canned chunks are never produced.
#[test]
fn cancel_aborts_and_nexts_done() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(
        200,
        vec![
            b"c0".to_vec(),
            b"c1".to_vec(),
            b"c2".to_vec(),
            b"c3".to_vec(),
        ],
    );

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__done = false;

        launch(function*() {
            const resp = yield* requestStream({ url: "http://test/cancel" });
            let r = yield resp.body.next();
            globalThis.__firstDone = r.done;
            resp.body.cancel();
            resp.body.cancel(); // idempotent
            let after = yield resp.body.next();
            globalThis.__afterCancel = after.done;
            globalThis.__done = true;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__firstDone"), "false");
    assert_eq!(app.eval_js("globalThis.__afterCancel"), "true");
    assert_eq!(
        app.http_stream_pulls(),
        1,
        "cancel must stop production: only the consumed chunk was pulled"
    );
}

/// The pull protocol is serial: a `next()` issued while a previous one is
/// still pending rejects with a clear error instead of lying `{done: true}`.
#[test]
fn concurrent_next_rejects() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(200, vec![b"one".to_vec(), b"two".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__done = false;
        globalThis.__p2 = "";

        launch(function*() {
            const resp = yield* requestStream({ url: "http://test/conc" });
            const p1 = resp.body.next();
            const p2 = resp.body.next();
            p2.then(v => { globalThis.__p2 = "resolved:" + v.done; },
                    e => { globalThis.__p2 = "rejected:" + e.message; });
            const r1 = yield p1;
            globalThis.__r1done = r1.done;
            globalThis.__done = true;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");
    // The p2 reaction is a microtask — give it its own pump-driven wait
    // rather than assuming queue ordering relative to __done.
    app.wait_for(|a| !a.eval_js("globalThis.__p2").is_empty());

    let p2 = app.eval_js("globalThis.__p2");
    assert!(
        p2.starts_with("rejected:"),
        "concurrent next() must reject with a clear message, got: {p2}"
    );
    assert!(p2.contains("still pending"), "{p2}");
    assert_eq!(app.eval_js("globalThis.__r1done"), "false");
}
