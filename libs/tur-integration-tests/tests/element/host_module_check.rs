//! Verify plugin-registered host modules (`tur:*`) resolve via the loader.

use boa_engine::{JsArgs, NativeFunction};
use tur_integration_tests::{HostExport, HostModulePlugin, TurTestApp};

#[test]
fn host_module_is_importable() {
    let plugin = HostModulePlugin {
        specifier: "tur:test",
        exports: vec![HostExport {
            name: "echo".to_string(),
            // Builder produces a fresh NativeFunction per instance (Phase 7:
            // `NativeFunction` is `!Send`, so we hold a Send+Sync builder
            // closure instead of a pre-built value).
            builder: Box::new(|_ctx| {
                NativeFunction::from_copy_closure(|_this, args, _ctx| {
                    Ok(args.get_or_undefined(0).clone())
                })
            }),
            length: 1,
        }],
    };
    let app = TurTestApp::new_with_extra_plugins(100.0, 100.0, vec![Box::new(plugin)]).unwrap();
    app.eval_module_source(r#"import { echo } from "tur:test"; globalThis.__r = echo(7);"#)
        .unwrap();
    assert_eq!(app.eval_js("String(globalThis.__r)"), "7");
}
