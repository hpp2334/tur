//! Verify embedder-registered host modules (`builtin:tur/*`) resolve via the loader.

use boa_engine::{JsArgs, NativeFunction};
use tur_integration_tests::TurTestApp;

#[test]
fn host_module_is_importable() {
    let mut app = TurTestApp::new(100.0, 100.0).unwrap();
    app.with_app(|tur| {
        let echo = NativeFunction::from_copy_closure(|_this, args, _ctx| {
            Ok(args.get_or_undefined(0).clone())
        });
        tur.register_host_module("builtin:tur/test", vec![("echo".to_string(), echo, 1)])
            .unwrap();
    });
    app.eval_module_source(r#"import { echo } from "builtin:tur/test"; globalThis.__r = echo(7);"#)
        .unwrap();
    assert_eq!(app.eval_js("String(globalThis.__r)"), "7");
}
