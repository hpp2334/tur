//! Experiment: can a host fn load an ES module RE-ENTRANTLY (while an outer
//! module is mid-evaluation)? This is what the playground's `compileCase`
//! needs: it runs inside the demo-impl module eval and must load user case
//! code as a module. Uses `&mut Context` directly (like `eval`), not
//! `&mut TurApp`, so it doesn't double-borrow.

use std::path::Path;

use boa_engine::{js_string, JsArgs, JsValue, Module, NativeFunction, Source};
use tur_integration_tests::TurTestApp;

#[test]
fn reentrant_module_load_via_host_fn() {
    let mut app = TurTestApp::new(100.0, 100.0).unwrap();
    app.with_app(|tur| {
        // `loadModule(src)` — parses + evaluates `src` as a module on the
        // current context (re-entrant safe if boa supports nested module eval).
        let load_module_fn = NativeFunction::from_copy_closure(|_this, args, ctx| {
            let src = args
                .get_or_undefined(0)
                .as_string()
                .ok_or_else(|| {
                    boa_engine::JsNativeError::typ()
                        .with_message("loadModule: expected string")
                })?
                .to_std_string_escaped();
            let module = Module::parse(
                Source::from_bytes(&src).with_path(Path::new("inner.mjs")),
                None,
                ctx,
            )?;
            let _ = module.load_link_evaluate(ctx);
            ctx.run_jobs()?;
            Ok(JsValue::undefined())
        });
        tur.register_host_module(
            "builtin:tur/cases",
            vec![("loadModule".to_string(), load_module_fn, 1)],
        )
        .unwrap();
    });

    // Outer module calls `loadModule(innerSrc)` during its own evaluation.
    // `innerSrc` imports `builtin:tur/std` (already registered) and stashes a value.
    app.eval_module_source(
        r#"
            import { loadModule } from "builtin:tur/cases";
            const inner = "import { source } from \"builtin:tur/std\"; globalThis.__inner = typeof source;";
            loadModule(inner);
            globalThis.__outer = "ran";
        "#,
    )
    .unwrap();

    assert_eq!(app.eval_js("globalThis.__outer"), "ran");
    assert_eq!(app.eval_js("globalThis.__inner"), "function");
    let _ = (js_string!(),);
}
