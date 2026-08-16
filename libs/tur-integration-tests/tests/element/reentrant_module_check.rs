//! Experiment: can a host fn load an ES module RE-ENTRANTLY (while an outer
//! module is mid-evaluation)? This is what the playground's `compileCase`
//! needs: it runs inside the demo-impl module eval and must load user case
//! code as a module. Uses `&mut Context` directly (like `eval`), not
//! `&mut TurApp`, so it doesn't double-borrow.

use std::path::Path;

use boa_engine::{JsArgs, JsValue, Module, NativeFunction, Source, js_string};
use tur_integration_tests::{NativeExport, NativeModulePlugin, TurTestApp};

#[test]
fn reentrant_module_load_via_host_fn() {
    // `loadModule(src)` — parses + evaluates `src` as a module on the
    // current context (re-entrant safe if boa supports nested module eval).
    // Built fresh per instance via the NativeExport builder (Phase 7).

    let plugin = NativeModulePlugin {
        specifier: "tur:cases",
        exports: vec![NativeExport {
            name: "loadModule".to_string(),
            // Builder produces a fresh NativeFunction per instance (Phase 7:
            // `NativeFunction` is `!Send`, so we hold a Send+Sync builder
            // closure instead of a pre-built value).
            builder: Box::new(|_ctx| {
                NativeFunction::from_copy_closure(|_this, args, ctx| {
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
                })
            }),
            length: 1,
        }],
    };
    let app = TurTestApp::new_with_extra_plugins(100.0, 100.0, vec![Box::new(plugin)]).unwrap();

    // Outer module calls `loadModule(innerSrc)` during its own evaluation.
    // `innerSrc` imports `tur:std` (already registered) and stashes a value.
    app.eval_module_source(
        r#"
            import { loadModule } from "tur:cases";
            const inner = "import { source } from \"tur:std\"; globalThis.__inner = typeof source;";
            loadModule(inner);
            globalThis.__outer = "ran";
        "#,
    )
    .unwrap();

    assert_eq!(app.eval_js("globalThis.__outer"), "ran");
    assert_eq!(app.eval_js("globalThis.__inner"), "function");
    let _ = (js_string!(),);
}
