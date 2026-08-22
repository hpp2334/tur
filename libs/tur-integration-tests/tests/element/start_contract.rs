//! The module lifecycle contract: `load_module` requires the module to
//! export `start()`. The engine calls the previous module's cleanup (the
//! value `start` returned) before evaluating the next module, clears any
//! leftover root tree, invokes `start`, and stores its return value as the
//! pending cleanup. The root-tree lifecycle is engine-owned: `mount`
//! replaces any existing root and teardown clears it — a module's cleanup
//! only handles its own non-tree resources (controllers, subscriptions).

use std::time::Duration;

use tur_integration_tests::TurTestApp;

#[test]
fn missing_start_export_fails_load() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    let err = app
        .load_module_raw(r#"import { Text } from "tur:std"; globalThis.__x = 1;"#)
        .expect_err("module without start export must fail");
    assert!(
        err.to_string().to_lowercase().contains("start"),
        "error should mention start: {err}"
    );
}

#[test]
fn start_is_called_and_cleanup_runs_before_reload() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        globalThis.__log = [];
        export function start() {
            globalThis.__log.push("start1");
            mount(Text({ text: "one", queryKey: ["one"] }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["one"]).is_some(),
        "module 1 tree mounted via start"
    );

    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start() {
            globalThis.__log.push("start2");
            mount(Text({ text: "two", queryKey: ["two"] }));
            return () => {
                globalThis.__log.push("cleanup2");
            };
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let log = app.eval_js("globalThis.__log.join(',')");
    assert_eq!(
        log, "start1,start2",
        "module 1 had no cleanup; module 2 start runs after teardown"
    );
    assert!(
        app.query_element(&["two"]).is_some(),
        "module 2 tree mounted"
    );
    assert!(
        app.query_element(&["one"]).is_none(),
        "module 1 root auto-cleared on re-load"
    );
}

/// A module's cleanup runs before the next module evaluates — observable
/// ordering even when the cleanup doesn't touch the tree.
#[test]
fn cleanup_ordering_across_reload() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        globalThis.__log = [];
        export function start() {
            globalThis.__log.push("start1");
            return () => {
                globalThis.__log.push("cleanup1");
            };
        }
    "#,
    )
    .unwrap();
    app.load_module_raw(
        r#"
        export function start() {
            globalThis.__log.push("start2");
        }
    "#,
    )
    .unwrap();
    let log = app.eval_js("globalThis.__log.join(',')");
    assert_eq!(
        log, "start1,cleanup1,start2",
        "cleanup1 must run before start2"
    );
}

/// Even when a module's cleanup forgets to unmount, a re-load must not leak
/// the old root: the engine clears any remaining root tree after running the
/// previous cleanup.
#[test]
fn reload_clears_leftover_root_tree() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start() {
            mount(Text({ text: "one", queryKey: ["one"] }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start() {
            mount(Text({ text: "two", queryKey: ["two"] }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        app.query_element(&["two"]).is_some(),
        "module 2 tree mounted"
    );
    assert!(
        app.query_element(&["one"]).is_none(),
        "module 1 root auto-cleared on re-load"
    );
}

/// Calling `mount` again within one module's lifetime replaces the root —
/// the old subtree must not linger.
#[test]
fn remount_replaces_previous_root() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        globalThis.__mount = mount;
        globalThis.__Text = Text;
        export function start({ store }) {
            globalThis.__store = store;
            mount(Text({ text: "first", queryKey: ["first"] }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["first"]).is_some());

    // Second mount via the stashed reference (same module still loaded).
    app.eval_js(
        r#"globalThis.__mount(globalThis.__Text({ text: "second", queryKey: ["second"] }));"#,
    );
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        app.query_element(&["second"]).is_some(),
        "second tree mounted"
    );
    assert!(
        app.query_element(&["first"]).is_none(),
        "first root replaced, not leaked"
    );
}

#[test]
fn start_throwing_fails_load() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    let err = app
        .load_module_raw(
            r#"
        export function start() {
            throw new Error("boom");
        }
    "#,
        )
        .expect_err("throwing start must fail the load");
    assert!(
        err.to_string().contains("boom"),
        "error should surface the thrown message: {err}"
    );
}

#[test]
fn non_function_start_export_fails_load() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    let err = app
        .load_module_raw("export const start = 42;")
        .expect_err("non-function start export must fail");
    assert!(
        err.to_string().to_lowercase().contains("start"),
        "error should mention start: {err}"
    );
}

#[test]
fn start_returning_non_function_is_ok() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start() {
            mount(Text({ text: "plain", queryKey: ["plain"] }));
            // No cleanup — returning undefined is fine.
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["plain"]).is_some());
}

/// The dev-tool root snapshot goes empty after a re-load into a startless
/// module and after the tree is replaced — engine-owned teardown.
#[test]
fn dev_tool_tree_reflects_reload() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start() {
            mount(Text({ text: "x" }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.dev_tool_element_tree().is_some());

    // Replace with a start-only module (no mount): teardown must clear the
    // tree, leaving the dev-tool snapshot empty.
    app.load_module_raw(
        r#"
        export function start() {}
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.dev_tool_element_tree().is_none(),
        "dev-tool tree must be empty after teardown without a new mount"
    );
}

// ---------------------------------------------------------------------------
// `start({ store })` — the engine creates the instance store and hands it to
// the module; the tree is instance-owned (born-bound to it at build).
// ---------------------------------------------------------------------------

/// `start` receives a working `{ store }`: a live `{get, set}` object the
/// module can read/write directly, and `mount(view)` (single arg) mounts
/// against the tree's bound instance store.
#[test]
fn start_receives_instance_store_and_single_arg_mount_works() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, source, Text } from "tur:std";
        globalThis.__n = source(7);
        export function start({ store }) {
            globalThis.__instance_store = store;
            store.set(globalThis.__n, 11);
            mount(Text({
                text: String(store.get(globalThis.__n)),
                queryKey: ["bound"],
            }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        app.query_element(&["bound"]).is_some(),
        "single-arg mount(view) must mount against the tree's bound instance store"
    );
    let v = app.eval_js("globalThis.__instance_store.get(globalThis.__n).toString()");
    assert_eq!(
        v, "11",
        "the start-injected store must be live (its own writes read back)"
    );
}

/// Across a reload, teardown clears the ROOT (not the tree): the next module
/// receives the same instance store and single-arg mounts into the same
/// instance-owned tree.
#[test]
fn reload_keeps_instance_tree_and_remounts_via_injected_store() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start({ store }) {
            globalThis.__instance_store = store;
            mount(Text({ text: "one", queryKey: ["one"] }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["one"]).is_some());

    app.load_module_raw(
        r#"
        import { mount, Text } from "tur:std";
        export function start({ store }) {
            if (store !== globalThis.__instance_store) {
                throw new Error("instance store identity changed across reload");
            }
            mount(Text({ text: "two", queryKey: ["two"] }));
        }
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        app.query_element(&["two"]).is_some(),
        "module 2 mounts into the same instance-owned tree"
    );
    assert!(
        app.query_element(&["one"]).is_none(),
        "module 1 root cleared on re-load"
    );
}
