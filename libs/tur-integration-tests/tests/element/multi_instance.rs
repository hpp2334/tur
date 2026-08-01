//! Multi-instance: one `TurRuntime` spawns multiple isolated `TurApp`
//! instances. Verifies JS-realm isolation (each instance has independent
//! global state) and shared-runtime semantics (same fonts/clock/capabilities).

use std::rc::Rc;

use boa_engine::context::time::FixedClock;
use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_native::NativeFontLoader;

/// Build a runtime with the std + animation plugins (no capabilities beyond
/// the defaults — instances are headless).
fn build_runtime() -> Rc<TurRuntime> {
    TurRuntime::builder()
        .font_loader(Rc::new(NativeFontLoader::new()))
        .clock(Rc::new(FixedClock::from_millis(0)))
        .plugin(TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .build()
        .expect("runtime build")
}

const SET_ID_JS: &str = r#"
    import { Text, render } from "tur:std";
    globalThis.__instanceId = "VALUE";
    render(Text({ text: "VALUE" }));
"#;

#[test]
fn instances_have_isolated_js_realms() {
    let runtime = build_runtime();
    let app_a = runtime.create_headless_app((100.0, 100.0)).expect("app A");
    let app_b = runtime.create_headless_app((100.0, 100.0)).expect("app B");

    // Load different state into each instance.
    app_a
        .load_module(SET_ID_JS.replace("VALUE", "A").as_str())
        .expect("load A");
    app_b
        .load_module(SET_ID_JS.replace("VALUE", "B").as_str())
        .expect("load B");

    // Each instance reads back its OWN global — they must differ.
    let id_a = app_a.eval_js("globalThis.__instanceId").expect("eval A");
    let id_b = app_b.eval_js("globalThis.__instanceId").expect("eval B");
    assert_eq!(id_a, "A", "instance A should have its own state");
    assert_eq!(id_b, "B", "instance B should have its own state");

    // Mutating A must not affect B.
    app_a.eval_js(r#"globalThis.__instanceId = "A2""#).unwrap();
    let id_b_after = app_b.eval_js("globalThis.__instanceId").unwrap();
    assert_eq!(id_b_after, "B", "instance B unaffected by A's mutation");
}

#[test]
fn instances_have_isolated_element_trees() {
    let runtime = build_runtime();
    let app_a = runtime.create_headless_app((100.0, 100.0)).expect("app A");
    let app_b = runtime.create_headless_app((100.0, 100.0)).expect("app B");

    // Mount a tree only in A.
    app_a
        .load_module(
            r#"
            import { Text, render } from "tur:std";
            render(Text({ text: "only-in-A", queryKey: ["a_only"] }));
        "#,
        )
        .expect("load A");
    app_a.run_frame().expect("frame A");

    // B has no tree mounted.
    let b_tree = app_b.dev_tool_element_tree();
    assert!(b_tree.is_none(), "instance B should have no tree");

    // A does have a tree.
    let a_tree = app_a.dev_tool_element_tree();
    assert!(a_tree.is_some(), "instance A should have a tree");
}

#[test]
fn headless_instance_runs_js_without_rendering() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless");

    // JS executes; a frame runs without panic even with a zero viewport.
    app.load_module(
        r#"
        import { source, get } from "tur:std";
        globalThis.__val = source(42);
        const v = get(globalThis.__val);
        globalThis.__readBack = v;
    "#,
    )
    .expect("load");
    app.run_frame().expect("frame");

    let val = app.eval_js("globalThis.__readBack").expect("eval");
    assert_eq!(val, "42", "headless instance ran JS");
}

#[test]
fn many_instances_share_one_runtime() {
    // Smoke test: spawn several instances from one runtime to confirm no
    // shared-state corruption (each gets its own boa Context + store).
    let runtime = build_runtime();
    let mut apps = Vec::new();
    for i in 0..5 {
        let app = runtime.create_headless_app((50.0, 50.0)).expect("app");
        app.load_module(format!(r#"globalThis.__idx = {i};"#).as_str())
            .expect("load");
        apps.push(app);
    }
    for (i, app) in apps.iter().enumerate() {
        let idx = app.eval_js("globalThis.__idx").expect("eval");
        assert_eq!(idx, i.to_string(), "instance {i} should have its own __idx");
    }
}
