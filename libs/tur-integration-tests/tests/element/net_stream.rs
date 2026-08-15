//! Integration tests for `tur:net.requestStream` — the async-iterable
//! streaming HTTP response.
//!
//! Validates the full path: JS calls `requestStream(opts)` → `RecordingHttp`
//! serves canned chunks → the async iterable yields `Uint8Array` chunks →
//! JS collects and concatenates them.

use tur_integration_tests::TurTestApp;

#[test]
fn stream_collects_multiple_chunks() {
    let app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

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
            const resp = yield requestStream({ url: "http://test/stream" });
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
    let app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    app.set_http_stream(200, vec![b"one chunk".to_vec()]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__collected = "";
        globalThis.__done = false;

        launch(function*() {
            const resp = yield requestStream({ url: "http://test/single" });

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
    let app = TurTestApp::new_with_http(200.0, 100.0).unwrap();

    // No chunks at all — body should immediately return {done: true}
    app.set_http_stream(204, vec![]);

    app.eval_module_source(
        r#"
        import { requestStream } from "tur:net";
        import { launch } from "tur:std";

        globalThis.__done = false;

        launch(function*() {
            const resp = yield requestStream({ url: "http://test/empty" });
            let result = yield resp.body.next();
            globalThis.__done = result.done;
        });
        "#,
    )
    .expect("module");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "true");
}
