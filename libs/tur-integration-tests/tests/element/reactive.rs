use tur_integration_tests::TurTestApp;

/// `set(mutation, ...args)` must invoke the mutation closure with
/// `(store_ctx, ...args)` — matching the dispatch contract used by the
/// event-flush path. Previously the store ctx was not prepended, causing
/// every arg to shift by one position and breaking any caller that relied on
/// the (ctx, ...args) signature (e.g. the complex-animation case's
/// `set(setSpeed, factor, label)` flow).
#[test]
fn set_mutation_receives_ctx_then_args() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_source(
        r#"
        const ctx = globalThis.__tur.__ctx;
        const t = globalThis.__tur;
        // Sink source — captures whatever the mutation writes.
        globalThis.__sink = t.source(ctx, "");

        // Mutation that receives (ctx, a, b) and writes a formatted string
        // to the sink so the test can read it back. If ctx is not prepended,
        // the closure would receive (a, b, undefined) instead.
        const m = t.mutate(ctx, (sctx, a, b) => {
            const hasCtx = sctx && typeof sctx === "object" && typeof sctx.get === "function";
            sctx.set(globalThis.__sink, (hasCtx ? "ctx-" : "noctx-") + a + "-" + b);
        });

        // Invoke via set(mutation, ...args) — the path that was buggy.
        t.set(ctx, m, "x", "y");
        "#,
    )
    .unwrap();
    app.render();

    let val = app.eval_js("globalThis.__tur.get(globalThis.__tur.__ctx, globalThis.__sink);");
    assert_eq!(val, "ctx-x-y",
        "set(m, x, y) should invoke m with (ctx, x, y) — got {val:?}");
}

/// Sanity check: `set(mutation, ...args)` with zero extra args still works
/// (the closure receives just the ctx). Guards against off-by-one regressions
/// in the prepend logic.
#[test]
fn set_mutation_with_zero_args_passes_ctx_only() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_source(
        r#"
        const ctx = globalThis.__tur.__ctx;
        const t = globalThis.__tur;
        globalThis.__sink = t.source(ctx, "");
        const m = t.mutate(ctx, (sctx) => {
            const hasCtx = sctx && typeof sctx === "object" && typeof sctx.get === "function";
            sctx.set(globalThis.__sink, hasCtx ? "ok" : "missing");
        });
        t.set(ctx, m);
        "#,
    )
    .unwrap();
    app.render();

    let val = app.eval_js("globalThis.__tur.get(globalThis.__tur.__ctx, globalThis.__sink);");
    assert_eq!(val, "ok",
        "set(m) should invoke m with just the ctx — got {val:?}");
}
