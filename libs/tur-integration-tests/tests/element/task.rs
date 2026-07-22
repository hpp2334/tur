//! Integration tests for the `sleep` + `launch` task primitives exported by
//! `tur:std` (the replacements for `setTimeout`/`setInterval`).
//!
//! These also exercise boa's generator support end-to-end — `launch` drives a
//! `function*` generator via `JsGenerator::next`, resuming it when each
//! `yield`ed promise (`sleep`) resolves.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

#[test]
fn sleep_resolves_after_delay() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { sleep } from "tur:std";
        globalThis.__done = "0";
        sleep(50).then(() => { globalThis.__done = "1"; });
        "#,
    )
    .unwrap();

    // Not resolved synchronously / after a single frame.
    app.settle();
    assert_eq!(app.eval_js("globalThis.__done"), "0");

    app.wait_for(|a| a.eval_js("globalThis.__done") == "1");
    assert_eq!(app.eval_js("globalThis.__done"), "1");
}

#[test]
fn launch_runs_generator_and_resumes_after_sleep() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch, sleep } from "tur:std";
        globalThis.__v = "0";
        launch(function* () {
            yield sleep(40);
            globalThis.__v = "7";
        });
        "#,
    )
    .unwrap();

    // The generator parks at its first `yield`; nothing has run yet.
    app.settle();
    assert_eq!(app.eval_js("globalThis.__v"), "0");

    app.wait_for(|a| a.eval_js("globalThis.__v") == "7");
    assert_eq!(app.eval_js("globalThis.__v"), "7");
}

#[test]
fn launch_repeating_loop_ticks_multiple_times() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch, sleep } from "tur:std";
        globalThis.__count = "0";
        launch(function* () {
            for (let i = 0; i < 3; i++) {
                yield sleep(40);
                globalThis.__count = String(i + 1);
            }
        });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__count") == "3");
    assert_eq!(app.eval_js("globalThis.__count"), "3");
}

#[test]
fn launch_cancel_stops_resumption() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch, sleep } from "tur:std";
        globalThis.__hit = "0";
        // The body after the `yield` must never run once we cancel.
        globalThis.__task = launch(function* () {
            yield sleep(120);
            globalThis.__hit = "1";
        });
        "#,
    )
    .unwrap();

    // Cancel before the sleep deadline elapses.
    app.eval_js("globalThis.__task.cancel()");

    // Advance well past the sleep deadline and settle: the in-flight sleep
    // resolves, but the driver ignores a cancelled task, so `__hit` stays 0.
    app.advance(Duration::from_millis(200)).unwrap();
    app.settle();

    assert_eq!(app.eval_js("globalThis.__hit"), "0");
}

#[test]
fn launch_debounce_supersedes_previous_task() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch, sleep } from "tur:std";
        globalThis.__calls = "0";
        globalThis.__pending = null;
        globalThis.__schedule = function () {
            if (globalThis.__pending) globalThis.__pending.cancel();
            globalThis.__pending = launch(function* () {
                yield sleep(60);
                globalThis.__calls = String(Number(globalThis.__calls) + 1);
            });
        };
        // Two rapid schedules: only the second should fire.
        globalThis.__schedule();
        globalThis.__schedule();
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__calls") == "1");
    assert_eq!(app.eval_js("globalThis.__calls"), "1");
}

/// Regression: `launch` must drive ANY iterator (native `function*` OR a
/// down-levelled tslib `_ts_generator` iterator), since rspack/swc bundles
/// transpile generators away. Simulate the transpiled shape: a plain function
/// returning an object whose `.next(value)` returns `{done, value}`.
#[test]
fn launch_drives_non_native_iterator() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch, sleep } from "tur:std";
        globalThis.__hit = "0";
        // A hand-written iterator that mimics what SWC/tslib produces for
        // `function* () { yield sleep(40); globalThis.__hit = "1"; }` — a
        // plain object with a `.next(value)` method, NOT a native Generator.
        function makeIter() {
            let state = 0;
            return {
                next: function () {
                    if (state === 0) { state = 1; return { value: sleep(40), done: false }; }
                    globalThis.__hit = "1";
                    return { value: undefined, done: true };
                },
            };
        }
        launch(makeIter);
        "#,
    )
    .unwrap();

    app.settle();
    assert_eq!(app.eval_js("globalThis.__hit"), "0");

    app.wait_for(|a| a.eval_js("globalThis.__hit") == "1");
    assert_eq!(app.eval_js("globalThis.__hit"), "1");
}

/// A rejected yielded promise surfaces as a thrown error at the `yield` point,
/// catchable with `try/catch` (the same ergonomics as `await`). After the
/// catch runs, the generator continues. Uses `Promise.reject`, which the frame
/// loop's JobQueue flush resolves exactly like a `sleep` resolution.
#[test]
fn launch_caught_rejection_resumes_generator() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch } from "tur:std";
        globalThis.__caught = "no";
        globalThis.__after = "no";
        launch(function* () {
            try {
                yield Promise.reject("boom");
                globalThis.__after = "unreachable";
            } catch (e) {
                globalThis.__caught = String(e);
            }
            globalThis.__after = "yes";
        });
        "#,
    )
    .unwrap();

    app.wait_for(|a| a.eval_js("globalThis.__after") == "yes");
    assert_eq!(app.eval_js("globalThis.__caught"), "boom");
    assert_eq!(app.eval_js("globalThis.__after"), "yes");
}

/// An uncaught rejection (no `try/catch` around the `yield`) must NOT panic:
/// the driver throws into the generator via `.throw`, the throw propagates back
/// out (nothing catches it), and the driver logs the uncaught rejection and
/// stops resuming. The body after the rejected `yield` never runs.
#[test]
fn launch_uncaught_rejection_stops_without_panic() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { launch } from "tur:std";
        globalThis.__reached = "no";
        launch(function* () {
            yield Promise.reject("boom");
            globalThis.__reached = "yes";
        });
        "#,
    )
    .unwrap();

    // Pump frames so the rejection handler fires; the driver must not panic.
    app.settle();
    app.advance(Duration::from_millis(50)).unwrap();
    app.settle();

    assert_eq!(app.eval_js("globalThis.__reached"), "no");
}
