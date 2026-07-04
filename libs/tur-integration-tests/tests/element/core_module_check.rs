//! Verify `builtin:tur/core` imports + renders via the module loader, and that
//! the native `Color` / `LinearGradient` const-objects + enums work. Exercises
//! the `eval_module_source` harness path.

use tur_integration_tests::TurTestApp;

#[test]
fn core_module_imports_and_renders() {
    let mut app = TurTestApp::new(400.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { Column, SizedBox, CrossAxisAlignment, render, view } from "builtin:tur/core";
            render(view(() =>
                Column({
                    crossAlignment: CrossAxisAlignment.Start,
                    children: [ SizedBox({ height: 50 }), SizedBox({ height: 30 }) ],
                })));
        "#,
    )
    .unwrap();
    app.render();
    for _ in 0..6 {
        let _ = app.tick();
    }

    let root = app.dev_tool_element_tree().expect("tree mounted");
    assert_eq!(root.name, "tur_flex");
    let inner = app.dev_tool_get_element(root.children[0]).expect("inner");
    assert_eq!(inner.name, "tur_flex");
    assert_eq!(inner.children.len(), 2, "inner Column should have two SizedBoxes");
}

/// Enums are exported from `builtin:tur/core` as TS-style numeric enums:
/// forward mapping (`MainAxisSize.Max === 0`) AND reverse mapping
/// (`MainAxisSize[0] === "Max"`). Both must hold at runtime.
#[test]
fn enum_dual_mapping() {
    let mut app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r#"
            import { MainAxisSize, BoxFit } from "builtin:tur/core";
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
    let mut app = TurTestApp::new(100.0, 100.0).unwrap();
    app.eval_module_source(
        r##"
            import { Color, LinearGradient } from "builtin:tur/core";
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
