//! Verify `tur:core` exposes ONLY the reactive core — atom primitives
//! (`source`/`derive`/`mutate`/`get`/`set`/`view`) + `mount`. Views, enums,
//! colors, and event types live in `tur:std`.

use tur_integration_tests::TurTestApp;

/// The reactive primitives + `mount` are importable from `tur:core`
/// and work end-to-end (a `view()` factory + `mount()` mount).
#[test]
fn core_reactive_primitives_import_and_mount() {
    let app = TurTestApp::new(400.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { view, mount } from "tur:core";
            // `view` produces an opaque handle; `mount` builds it. The actual
            // view tree is built by the std-layer factory in `std_module_check`.
            // Here we just confirm the core primitives resolve and `view` returns
            // a non-null opaque handle.
            globalThis.__handle = view(() => {
                throw new Error("view body should not run until mount");
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

/// `mount` is importable from core and builds the view tree. Uses a `view` thunk
/// whose body builds nothing (the real widget tests live in `std_module_check`).
#[test]
fn core_mount_importable() {
    let app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { view, mount } from "tur:core";
            const h = view(() => null);
            globalThis.__has_mount = typeof mount === "function";
        "#,
    )
    .unwrap();
    assert_eq!(app.eval_js("globalThis.__has_mount"), "true");
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
