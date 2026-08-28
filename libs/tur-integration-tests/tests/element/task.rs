//! Integration tests for the `sleep` Task + the async/await model exported
//! by `tur:std` (the replacement for `setTimeout`/`setInterval`).
//!
//! Every async engine API returns `Task<T> = { promise, cancel() }` — the
//! shape/cancel/debounce semantics are pinned in `task_promise.rs`, and the
//! completion → PromiseJob → reactive-set path is pinned in
//! `async_bridge.rs`. This file pins the *composition* layer: plain
//! `async`/`await` functions driven by boa's microtask queue, with loop
//! cancellation via the current sleep's `cancel()` + `isCancelError`.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// An async loop ticks repeatedly — each `await sleep(ms).promise` parks and
/// resumes inside the engine's flush loop.
#[test]
fn async_loop_ticks_multiple_times() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        globalThis.__count = "0";
        (async () => {
            for (let i = 0; i < 3; i++) {
                await sleep(40).promise;
                globalThis.__count = String(i + 1);
            }
        })();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__count") == "3");
    assert_eq!(app.eval_js("globalThis.__count"), "3");
}

/// The ticker idiom: an async loop that stops when its current sleep is
/// cancelled — the `await` throws `CancelError`, the `isCancelError` catch
/// returns, and nothing after the cancelled await ever runs.
#[test]
fn async_loop_stops_when_current_sleep_cancelled() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { isCancelError, sleep } from "tur:std";
        globalThis.__hit = "0";
        globalThis.__tick = null;
        (async () => {
            try {
                for (;;) {
                    globalThis.__tick = sleep(80);
                    await globalThis.__tick.promise;
                    globalThis.__hit = "tick-ran";
                }
            } catch (e) {
                if (!isCancelError(e)) throw e;
            }
            globalThis.__hit = "stopped";
        })();
        "#,
    )
    .unwrap();

    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.eval_js("globalThis.__hit"), "0");
    app.eval_js("globalThis.__tick.cancel()");
    app.wait_for(|a| a.eval_js("globalThis.__hit") == "stopped");

    // Advance well past the sleep deadline: the loop body never runs again.
    app.wait_for_timeout(Duration::from_millis(200));
    assert_eq!(app.eval_js("globalThis.__hit"), "stopped");
}

/// A rejected awaited promise surfaces as a thrown error at the `await`
/// point, catchable with `try/catch` (native async semantics — boa's
/// microtask queue drives it exactly like a sleep resolution). After the
/// catch runs, the async fn continues.
#[test]
fn caught_rejection_resumes_async_fn() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        globalThis.__caught = "no";
        globalThis.__after = "no";
        (async () => {
            try {
                await Promise.reject("boom");
                globalThis.__after = "unreachable";
            } catch (e) {
                globalThis.__caught = String(e);
            }
            globalThis.__after = "yes";
        })();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__after") == "yes");
    assert_eq!(app.eval_js("globalThis.__caught"), "boom");
    assert_eq!(app.eval_js("globalThis.__after"), "yes");
}

/// An uncaught rejection inside an async fn must not panic the engine —
/// boa settles the rejected promise and the continuation simply never
/// runs. (Unhandled-rejection reporting is the host's business.)
#[test]
fn uncaught_rejection_in_async_fn_stops_without_panic() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        globalThis.__reached = "no";
        (async () => {
            await Promise.reject("boom");
            globalThis.__reached = "yes";
        })();
        "#,
    )
    .unwrap();

    // Pump frames so the rejection lands; the engine must not panic.
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.wait_for_timeout(Duration::from_millis(50));

    assert_eq!(app.eval_js("globalThis.__reached"), "no");
}
