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
    app.eval_module_source(
        r#"
        import { source, mutate, set } from "tur:std";
        // Sink source — captures whatever the mutation writes.
        globalThis.__sink = source("");

        // Mutation that receives (ctx, a, b) and writes a formatted string
        // to the sink so the test can read it back. If ctx is not prepended,
        // the closure would receive (a, b, undefined) instead.
        const m = mutate((sctx, a, b) => {
            const hasCtx = sctx && typeof sctx === "object" && typeof sctx.get === "function";
            sctx.set(globalThis.__sink, (hasCtx ? "ctx-" : "noctx-") + a + "-" + b);
        });

        // Invoke via set(mutation, ...args) — the path that was buggy.
        set(m, "x", "y");
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.eval_module_source(
        r#"import { get } from "tur:std"; globalThis.__result = get(globalThis.__sink);"#,
    )
    .unwrap();
    let val = app.eval_js("globalThis.__result");
    assert_eq!(
        val, "ctx-x-y",
        "set(m, x, y) should invoke m with (ctx, x, y) — got {val:?}"
    );
}

/// Sanity check: `set(mutation, ...args)` with zero extra args still works
/// (the closure receives just the ctx). Guards against off-by-one regressions
/// in the prepend logic.
#[test]
fn set_mutation_with_zero_args_passes_ctx_only() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { source, mutate, set } from "tur:std";
        globalThis.__sink = source("");
        const m = mutate((sctx) => {
            const hasCtx = sctx && typeof sctx === "object" && typeof sctx.get === "function";
            sctx.set(globalThis.__sink, hasCtx ? "ok" : "missing");
        });
        set(m);
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    app.eval_module_source(
        r#"import { get } from "tur:std"; globalThis.__result = get(globalThis.__sink);"#,
    )
    .unwrap();
    let val = app.eval_js("globalThis.__result");
    assert_eq!(
        val, "ok",
        "set(m) should invoke m with just the ctx — got {val:?}"
    );
}
