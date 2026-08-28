//! Integration tests for `tur:net.requestStream` — the Task-based streaming
//! HTTP request (`{ promise, cancel() }`) with an async-iterable response
//! body.
//!
//! Validates the full path: `await requestStream(opts).promise` →
//! `RecordingHttp` serves canned chunks → the async iterable yields
//! `Uint8Array` chunks → JS collects and concatenates them. Plus the
//! backpressure surface: pull-only-what's-consumed, `backpressure`
//! validation, `task.cancel()` wire aborts, and the serial-`next()`
//! protocol. (Cancel semantics live in `task_promise.rs`.)

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

        globalThis.__collected = "";
        globalThis.__done = false;

        (async () => {
            const resp = await requestStream({ url: "http://test/stream" }).promise;
            globalThis.__status = resp.status;

            let result = await resp.body.next();
            while (!result.done) {
                const text = Array.from(result.value)
                    .map(b => String.fromCharCode(b))
                    .join("");
                globalThis.__collected += text;
                result = await resp.body.next();
            }
            globalThis.__done = true;
        })();
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

        globalThis.__collected = "";
        globalThis.__done = false;

        (async () => {
            const resp = await requestStream({ url: "http://test/single" }).promise;

            let result = await resp.body.next();
            while (!result.done) {
                const text = Array.from(result.value)
                    .map(b => String.fromCharCode(b))
                    .join("");
                globalThis.__collected += text;
                result = await resp.body.next();
            }
            globalThis.__done = true;
        })();
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

        globalThis.__done = false;

        (async () => {
            const resp = await requestStream({ url: "http://test/empty" }).promise;
            let result = await resp.body.next();
            globalThis.__done = result.done;
        })();
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

        globalThis.__done = false;

        (async () => {
            const resp = await requestStream({ url: "http://test/pull" }).promise;
            let r = await resp.body.next();
            globalThis.__first = r.done ? "" : Array.from(r.value).length;
            r = await resp.body.next();
            globalThis.__second = !r.done;
            globalThis.__done = true;
        })();
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

/// `backpressure` is validated at parse time: bad values throw at the
/// `await` with a clear message. (See the parse-time rejection path in
/// `tur_net_request_stream`.)
#[test]
fn stream_backpressure_validation() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(200, vec![b"x".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";

        globalThis.__done = false;

        (async () => {
            const cases = [
                [{ url: "http://test/bb0", backpressure: { value: 0, unit: "KB" } },
                 "backpressure.value must be >= 1"],
                [{ url: "http://test/bb1", backpressure: { value: 64, unit: "XX" } },
                 'backpressure.unit must be one of "B" | "KB" | "MB" | "GB"'],
                [{ url: "http://test/bb2", backpressure: 64 },
                 "backpressure must be an object: { value, unit }"],
            ];
            for (const [opts, want] of cases) {
                try {
                    await requestStream(opts).promise;
                    globalThis.__caught = "unreachable";
                } catch (e) {
                    globalThis.__caught = String(e.message);
                }
                if (globalThis.__caught !== want) {
                    globalThis.__mismatch = globalThis.__caught + " != " + want;
                    break;
                }
            }
            globalThis.__done = true;
        })();
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(
        app.eval_js("globalThis.__mismatch"),
        "undefined",
        "every backpressure validation case must match its expected message"
    );
}

/// `backpressure` has **no upper cap**: a 2 GiB budget (far above the former
/// 64 MiB guard) is accepted and the stream completes normally.
#[test]
fn stream_backpressure_accepts_large_budget() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(200, vec![b"chunk-0!!".to_vec(), b"chunk-1!!".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";

        globalThis.__done = false;

        (async () => {
            const t = requestStream({
                url: "http://test/big",
                backpressure: { value: 2, unit: "GB" },
            });
            const resp = await t.promise;
            let collected = 0;
            let r = await resp.body.next();
            while (!r.done) {
                collected += r.value.length;
                r = await resp.body.next();
            }
            globalThis.__collected = String(collected);
            globalThis.__done = true;
        })();
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");
    assert_eq!(app.eval_js("globalThis.__collected"), "18");
}

/// `task.cancel()` wire-aborts deterministically: pending/subsequent
/// `next()` steps resolve `{ done: true }`, the producer is no longer
/// pulled, and (the response having already resolved) the cancel is
/// abort-only for the promise.
#[test]
fn stream_task_cancel_aborts_and_nexts_done() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(
        200,
        vec![
            b"chunk-0".to_vec(),
            b"chunk-1".to_vec(),
            b"chunk-2".to_vec(),
            b"chunk-3".to_vec(),
        ],
    );

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";

        globalThis.__done = false;

        (async () => {
            const t = requestStream({ url: "http://test/cancel" });
            globalThis.__t = t;
            const resp = await t.promise;
            const first = await resp.body.next();
            globalThis.__firstDone = String(first.done);
            const pending = resp.body.next();
            t.cancel();
            t.cancel(); // idempotent
            const after = await pending;
            globalThis.__afterDone = String(after.done);
            const later = await resp.body.next();
            globalThis.__laterDone = String(later.done);
            globalThis.__done = true;
        })();
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__firstDone"), "false");
    assert_eq!(app.eval_js("globalThis.__afterDone"), "true");
    assert_eq!(app.eval_js("globalThis.__laterDone"), "true");
    // Two pulls (first + the cancelled pending); the abort stops the producer.
    assert_eq!(app.http_stream_pulls(), 2);
}

/// The pull protocol is serial: a `next()` issued while a previous one is
/// still pending rejects — that's what carries backpressure. The natural
/// promise shape: keep the first call's promise, `.then` the second.
#[test]
fn concurrent_next_rejects() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    // A stream that never produces a chunk on its own: each pull parks until
    // the RecordingHttp stream is polled... canned chunks are pulled eagerly
    // per next(), so issue both calls before awaiting either.
    app.set_http_stream(200, vec![b"chunk-0".to_vec(), b"chunk-1".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";

        globalThis.__done = false;

        (async () => {
            const resp = await requestStream({ url: "http://test/conc" }).promise;
            const p1 = resp.body.next();
            const p2 = resp.body.next();
            p2.then(
                () => { globalThis.__p2 = "resolved"; },
                (e) => { globalThis.__p2 = "rejected:" + e.message; },
            );
            await p1;
            globalThis.__p1ok = "yes";
            globalThis.__done = true;
        })();
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__p1ok"), "yes");
    assert_eq!(
        app.eval_js("globalThis.__p2"),
        "rejected:stream.next() called while a previous call is still pending"
    );
}

/// `for await ... of resp.body` — the async-iterator surface — drains the
/// stream to completion.
#[test]
fn stream_for_await_drains() {
    let mut app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(
        200,
        vec![b"for-".to_vec(), b"await".to_vec(), b"-works".to_vec()],
    );

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";

        globalThis.__collected = "";
        globalThis.__done = false;

        (async () => {
            const resp = await requestStream({ url: "http://test/forawait" }).promise;
            for await (const chunk of resp.body) {
                globalThis.__collected += String.fromCharCode(...chunk);
            }
            globalThis.__done = true;
        })();
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");

    assert_eq!(app.eval_js("globalThis.__collected"), "for-await-works");
}
