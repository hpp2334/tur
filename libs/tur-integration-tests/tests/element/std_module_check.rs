//! Verify `tur:std` — the widget library. It re-exports the reactive
//! core (`source`/`derive`/`mutate`/`get`/`set`/`view`/`render`) and adds views,
//! enums, colors, controllers, resources, and event types. Exercises the
//! `eval_module_source` harness path.

use tur_integration_tests::TurTestApp;

/// Views + reactive primitives import from a single module (`tur:std`)
/// and render end-to-end. This is the canonical "std is the convenience
/// superset" smoke test.
#[test]
fn std_module_imports_and_renders() {
    let app = TurTestApp::new(400.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { Column, SizedBox, CrossAxisAlignment, setViewRoot, viewRoot, view } from "tur:std";
            setViewRoot(viewRoot("main"), view(() =>
                Column({
                    crossAlignment: CrossAxisAlignment.Start,
                    children: [ SizedBox({ height: 50 }), SizedBox({ height: 30 }) ],
                })));
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    for _ in 0..6 {
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }

    let root = app.dev_tool_element_tree().expect("tree mounted");
    assert_eq!(
        root.name, "tur_root",
        "root is the engine's generic wrapper"
    );
    let inner = app.dev_tool_get_element(root.children[0]).expect("inner");
    assert_eq!(inner.name, "tur_flex");
    assert_eq!(
        inner.children.len(),
        2,
        "inner Column should have two SizedBoxes"
    );
}

/// Core primitives are re-exported from std (`source` resolves from
/// `tur:std` even though its canonical home is `tur:std`).
#[test]
fn std_re_exports_core_primitives() {
    let app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { source, get, set } from "tur:std";
            const a = source(42);
            globalThis.__val = get(a);
            set(a, 7);
            globalThis.__val2 = get(a);
        "#,
    )
    .unwrap();
    assert_eq!(app.eval_js("globalThis.__val"), "42");
    assert_eq!(app.eval_js("globalThis.__val2"), "7");
}

/// Enums are exported from `tur:std` as TS-style numeric enums:
/// forward mapping (`MainAxisSize.Max === 0`) AND reverse mapping
/// (`MainAxisSize[0] === "Max"`). Both must hold at runtime.
#[test]
fn enum_dual_mapping() {
    let app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { MainAxisSize, BoxFit } from "tur:std";
            globalThis.__fwd = MainAxisSize.Max === 0 && BoxFit.Cover === 2;
            globalThis.__rev = MainAxisSize[0] === "Max" && BoxFit[2] === "Cover";
        "#,
    )
    .unwrap();
    assert_eq!(app.eval_js("globalThis.__fwd"), "true", "forward mapping");
    assert_eq!(app.eval_js("globalThis.__rev"), "true", "reverse mapping");
}

/// `Color` and `LinearGradient` are exported as native const-objects whose
/// static builder methods return Rust-owned color/brush opaques. The returned
/// values must be non-null objects (the opaques); this is the smoke test for
/// the `bound_native`-method wiring in `init_bridge`.
#[test]
fn native_color_and_gradient_builders() {
    let app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r##"
            import { Color, LinearGradient } from "tur:std";
            const c1 = Color.hex("#ff0000");
            const c2 = Color.rgb(0, 255, 0);
            const c3 = Color.rgba(0, 0, 255, 128);
            const g = LinearGradient.create({
                start: [0, 0], end: [1, 1],
                stops: [ { offset: 0, color: c1 }, { offset: 1, color: c3 } ],
            });
            globalThis.__ok =
                typeof c1 === "object" && c1 !== null &&
                typeof c2 === "object" && c2 !== null &&
                typeof c3 === "object" && c3 !== null &&
                typeof g === "object" && g !== null;
        "##,
    )
    .unwrap();
    assert_eq!(
        app.eval_js("globalThis.__ok"),
        "true",
        "Color/LinearGradient builders should return opaque objects"
    );
}
