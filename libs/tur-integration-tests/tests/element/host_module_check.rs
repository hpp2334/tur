//! Verify plugin-registered host modules (`builtin:tur/*`) resolve via the loader.

use boa_engine::{JsArgs, NativeFunction};
use tur_integration_tests::{HostModulePlugin, TurTestApp};

#[test]
fn host_module_is_importable() {
    let echo = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        Ok(args.get_or_undefined(0).clone())
    });
    let plugin = HostModulePlugin {
        specifier: "builtin:tur/test",
        exports: vec![("echo".to_string(), echo, 1)],
    };
    let app = TurTestApp::new_with_extra_plugins(
        100.0,
        100.0,
        vec![Box::new(plugin)],
    )
    .unwrap();
    app.eval_module_source(r#"import { echo } from "builtin:tur/test"; globalThis.__r = echo(7);"#)
        .unwrap();
    assert_eq!(app.eval_js("String(globalThis.__r)"), "7");
}
