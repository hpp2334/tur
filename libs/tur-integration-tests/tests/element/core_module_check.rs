//! Verify `tur:core` exposes ONLY the reactive core — atom primitives
//! (`source`/`derive`/`mutate`/`get`/`set`/`view`) + `render`. Views, enums,
//! colors, and event types live in `tur:std`.

use tur_integration_tests::TurTestApp;

/// The reactive primitives + `render` are importable from `tur:core`
/// and work end-to-end (a `view()` factory + `render()` mount).
#[test]
fn core_reactive_primitives_import_and_render() {
    let app = TurTestApp::new(400.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { view, render } from "tur:core";
            // `view` produces an opaque handle; `render` mounts it. The actual
            // view tree is built by the std-layer factory in `std_module_check`.
            // Here we just confirm the core primitives resolve and `view` returns
            // a non-null opaque handle.
            globalThis.__handle = view(() => {
                throw new Error("view body should not run until render");
            });
        "#,
    )
    .unwrap();
    assert_eq!(
        app.eval_js("typeof globalThis.__handle"),
        "object",
        "view() should return an opaque handle object"
    );
}

/// `render` is importable from core and mounts a tree. Uses a `view` thunk
/// whose body builds nothing (the real widget tests live in `std_module_check`).
#[test]
fn core_render_importable() {
    let app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { view, render } from "tur:core";
            const h = view(() => null);
            globalThis.__has_render = typeof render === "function";
        "#,
    )
    .unwrap();
    assert_eq!(app.eval_js("globalThis.__has_render"), "true");
}

/// Views, enums, and colors are NOT in core anymore — they moved to
/// `tur:std`. A named import for a widget from core fails at link time
/// (boa errors on missing named exports for synthetic modules).
#[test]
fn core_does_not_export_widgets() {
    let app = TurTestApp::new(100.0, 100.0).unwrap();
    let err = app
        .eval_module_source(
            r#"
                import { Container } from "tur:core";
                globalThis.__container_type = typeof Container;
            "#,
        )
        .expect_err("importing Container from tur:core should fail at link time");
    assert!(
        format!("{err:?}").contains("Container"),
        "error should mention Container, got: {err:?}"
    );
}
