use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// Common scaffold: a mounted trivial tree + a store stashed on globalThis.
/// The caller's extra body declares the atoms/watchers and kicks start$.
fn setup(app: &mut TurTestApp, extra: &str) {
    app.eval_module_source(&format!(
        r#"
        import {{ Container, createStore, mount, mutate, source, watch }} from "tur:std";
        const store = createStore();
        globalThis.__store = store;
        globalThis.__n = source(0);
        globalThis.__fires = source(0);
        globalThis.__caught = source("unset");
        {extra}
        mount(store, Container({{ children: [] }}));
        "#,
        extra = extra,
    ))
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
}

fn js(app: &TurTestApp, expr: &str) -> String {
    app.eval_js(expr)
}

/// `watch` fires the callback when the watched SOURCE changes, after start$.
#[test]
fn watch_fires_on_source_change_after_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        const { start$ } = watch(globalThis.__n, mutate((ctx) => {
            ctx.set(globalThis.__fires, ctx.get(globalThis.__fires) + 1);
        }));
        globalThis.__start = start$;
        store.set(start$);
        "#,
    );

    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "0"
    );
    js(&app, "globalThis.__store.set(globalThis.__n, 5)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "1",
        "watcher should fire exactly once for one source change"
    );
    js(&app, "globalThis.__store.set(globalThis.__n, 6)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "2",
        "watcher should fire again on a second change"
    );
}

/// Change-only semantics: start$ must NOT fire the callback (no bootstrap).
#[test]
fn watch_change_only_no_fire_on_start() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        const { start$ } = watch(globalThis.__n, mutate((ctx) => {
            ctx.set(globalThis.__fires, ctx.get(globalThis.__fires) + 1);
        }));
        globalThis.__start = start$;
        store.set(start$);
        store.set(start$); // idempotent double start — still no fire
        "#,
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "0"
    );
}

/// No delivery before start$, none after stop$.
#[test]
fn watch_no_fire_before_start_or_after_stop() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        const { start$, stop$ } = watch(globalThis.__n, mutate((ctx) => {
            ctx.set(globalThis.__fires, ctx.get(globalThis.__fires) + 1);
        }));
        globalThis.__start = start$;
        globalThis.__stop = stop$;
        "#,
    );

    // Before start: changes are not delivered.
    js(&app, "globalThis.__store.set(globalThis.__n, 5)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "0"
    );

    // After start: delivered.
    js(&app, "globalThis.__store.set(globalThis.__start)");
    app.wait_for_timeout(Duration::ZERO);
    js(&app, "globalThis.__store.set(globalThis.__n, 6)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "1"
    );

    // After stop: not delivered again.
    js(&app, "globalThis.__store.set(globalThis.__stop)");
    app.wait_for_timeout(Duration::ZERO);
    js(&app, "globalThis.__store.set(globalThis.__n, 7)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "1"
    );
}

/// Watching a DERIVED fires when one of its deps is written, and the
/// callback reads the recomputed value.
#[test]
fn watch_fires_on_derived_when_dep_written() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        import { derive } from "tur:std";
        globalThis.__d = derive((ctx) => ctx.get(globalThis.__n) * 2);
        const { start$ } = watch(globalThis.__d, mutate((ctx) => {
            ctx.set(globalThis.__fires, ctx.get(globalThis.__d));
        }));
        globalThis.__start = start$;
        store.set(start$);
        "#,
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "0"
    );
    js(&app, "globalThis.__store.set(globalThis.__n, 21)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "42",
        "watcher on derived must fire and read the recomputed value"
    );
}

/// Same-value writes are equality-gated — no spurious delivery.
#[test]
fn watch_same_value_write_does_not_fire() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        const { start$ } = watch(globalThis.__n, mutate((ctx) => {
            ctx.set(globalThis.__fires, ctx.get(globalThis.__fires) + 1);
        }));
        store.set(start$);
        "#,
    );
    js(&app, "globalThis.__store.set(globalThis.__n, 0)"); // unchanged value
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "0"
    );
}

/// Loop detection: a callback that writes the watched SOURCE throws a JS
/// error at the call site, the write is rejected, and the engine keeps
/// flushing (later changes still deliver).
#[test]
fn watch_self_invalidation_of_source_throws() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        const { start$ } = watch(globalThis.__n, mutate((ctx) => {
            try {
                ctx.set(globalThis.__n, 999);
                ctx.set(globalThis.__caught, "no-throw");
            } catch (e) {
                ctx.set(globalThis.__caught, String(e));
            }
            ctx.set(globalThis.__fires, ctx.get(globalThis.__fires) + 1);
        }));
        store.set(start$);
        "#,
    );

    js(&app, "globalThis.__store.set(globalThis.__n, 5)");
    app.wait_for_timeout(Duration::ZERO);

    let caught = js(&app, "globalThis.__store.get(globalThis.__caught)");
    assert!(
        caught.contains("watch loop"),
        "self-invalidation must throw a watch-loop error at the call site, got: {caught}"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__n))"),
        "5",
        "the looping write must be rejected (no state change)"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "1",
        "the callback still completes after catching"
    );

    // Engine stays healthy: a later change still delivers exactly once.
    js(&app, "globalThis.__store.set(globalThis.__n, 7)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "2"
    );
}

/// Loop detection through a derived: the callback writes a DEP of the
/// watched derived (re-invalidating it) — must throw too.
#[test]
fn watch_self_invalidation_of_derived_dep_throws() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        import { derive } from "tur:std";
        globalThis.__d = derive((ctx) => ctx.get(globalThis.__n) * 2);
        const { start$ } = watch(globalThis.__d, mutate((ctx) => {
            try {
                ctx.set(globalThis.__n, 1);
                ctx.set(globalThis.__caught, "no-throw");
            } catch (e) {
                ctx.set(globalThis.__caught, String(e));
            }
        }));
        store.set(start$);
        "#,
    );

    js(&app, "globalThis.__store.set(globalThis.__n, 5)");
    app.wait_for_timeout(Duration::ZERO);

    let caught = js(&app, "globalThis.__store.get(globalThis.__caught)");
    assert!(
        caught.contains("watch loop"),
        "writing a dep of the watched derived must throw, got: {caught}"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__n))"),
        "5",
        "the looping write must be rejected"
    );
}

/// Same-frame coalescing: two changes to the watched atom within one flush
/// (both written inside a single mutation) deliver the callback exactly once.
#[test]
fn watch_coalesces_same_frame_changes() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    setup(
        &mut app,
        r#"
        const { start$ } = watch(globalThis.__n, mutate((ctx) => {
            ctx.set(globalThis.__fires, ctx.get(globalThis.__fires) + 1);
        }));
        store.set(start$);
        globalThis.__double = mutate((ctx) => {
            ctx.set(globalThis.__n, 1);
            ctx.set(globalThis.__n, 2);
        });
        "#,
    );
    js(&app, "globalThis.__store.set(globalThis.__double)");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fires))"),
        "1",
        "two same-frame writes must coalesce to one delivery"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__n))"),
        "2"
    );
}

/// Convergence backstop: two watchers ping-ponging through each other's
/// atoms (A watches a and writes b; B watches b and writes a) must terminate
/// within the frame — each watcher delivers at most once per frame, so the
/// cycle cannot spin the fixed-point loop forever.
#[test]
fn watch_pingpong_terminates_within_frame() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            Container,
            createStore,
            mount,
            mutate,
            source,
            watch,
        } from "tur:std";
        const store = createStore();
        globalThis.__store = store;
        globalThis.__a = source(0);
        globalThis.__b = source(0);
        globalThis.__fa = source(0);
        globalThis.__fb = source(0);
        const wa = watch(globalThis.__a, mutate((ctx) => {
            ctx.set(globalThis.__fa, ctx.get(globalThis.__fa) + 1);
            ctx.set(globalThis.__b, ctx.get(globalThis.__b) + 1);
        }));
        const wb = watch(globalThis.__b, mutate((ctx) => {
            ctx.set(globalThis.__fb, ctx.get(globalThis.__fb) + 1);
            ctx.set(globalThis.__a, ctx.get(globalThis.__a) + 10);
        }));
        store.set(wa.start$);
        store.set(wb.start$);
        mount(store, Container({ children: [] }));
        "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    js(&app, "globalThis.__store.set(globalThis.__a, 1)");
    app.wait_for_timeout(Duration::ZERO);

    // A fires (b += 1), B fires (a += 10), A is done for this frame → stop.
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__a))"),
        "11"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__b))"),
        "1"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fa))"),
        "1"
    );
    assert_eq!(
        js(&app, "String(globalThis.__store.get(globalThis.__fb))"),
        "1"
    );
}
