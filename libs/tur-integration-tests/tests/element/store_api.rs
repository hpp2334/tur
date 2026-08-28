use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// The instance store materializes a never-touched source declaration on
/// first `get`, seeding it with the declared initial value. Declarations
/// carry no state; the store is the KV.
#[test]
fn store_get_materializes_with_initial_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { source } from "tur:std";
        globalThis.__s = store;
        globalThis.__n = source(42);
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let val = app.eval_js("globalThis.__s.get(globalThis.__n).toString()");
    assert_eq!(
        val, "42",
        "first get should materialize with the initial value"
    );
}

/// One instance, one store: `createStore` is NOT exported (import fails at
/// link time) — the only store is the engine-created instance store handed
/// to `start({ store })`. Same for the free module-level `get`/`set`.
#[test]
fn create_store_and_free_get_set_are_not_exported() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    let create_gone = app
        .load_module_raw(r#"import { createStore } from "tur:std"; export function start() {}"#)
        .is_err();
    assert!(
        create_gone,
        "importing `createStore` from tur:std must fail at link time"
    );

    let free_get_gone = app
        .load_module_raw(r#"import { get } from "tur:std"; export function start() {}"#)
        .is_err();
    assert!(
        free_get_gone,
        "importing free `get` from tur:std must fail at link time"
    );
    let free_set_gone = app
        .load_module_raw(r#"import { set } from "tur:std"; export function start() {}"#)
        .is_err();
    assert!(
        free_set_gone,
        "importing free `set` from tur:std must fail at link time"
    );
}

/// `mount(view)` mounts against the instance store; the free module-level
/// `get`/`set` exports are gone (import fails at link time).
#[test]
fn mount_view_renders() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    app.eval_module_source(
        r#"
        import { mount, view, Text } from "tur:std";
        mount(view(() => Text({ text: "hello-store" })));
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let tree = app.dev_tool_element_tree().expect("root must exist");
    assert_eq!(tree.name, "tur_root");
    assert!(
        !tree.children.is_empty(),
        "root should have the mounted view as its child"
    );
    let text = app
        .dev_tool_get_element(tree.children[0])
        .expect("text child");
    assert!(
        text.name.contains("text") || text.name.contains("paragraph"),
        "mounted child should be a text element, got {}",
        text.name
    );
}

/// Tree-driven declarations (props) materialize into the instance store: a
/// `store.get` on the same declaration reads the same atom.
#[test]
fn prop_declarations_share_the_instance_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { mount, mutate, source, view, PointerInteract, Text } from "tur:std";
        const count = source(0);
        globalThis.__count = count;
        const bump = mutate((ctx) => ctx.set(count, 99));
        globalThis.__bump = bump;
        const clickBump = mutate((ctx) => ctx.set(count, 55));
        const ui = view(() => PointerInteract({
            onClick: clickBump,
            child: Text({ text: "click-me" }),
        }));
        export function start({ store }) {
            globalThis.__store = store;
            mount(ui);
        }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Invoke the same mutation decl the click uses, through the store —
    // verifies the decl materializes into the instance store's KV.
    app.eval_js("globalThis.__store.set(globalThis.__bump); 'ok'");
    app.wait_for_timeout(std::time::Duration::ZERO);
    let val = app.eval_js("globalThis.__store.get(globalThis.__count).toString()");
    assert_eq!(
        val, "99",
        "store.set(mutation) must hit the instance store's atom"
    );

    // Now the click path: the pointer event enqueues the same decl, which the
    // flush resolves against the instance store.
    let (cx, cy) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let pointer = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        app.get_element_absolute_bounds(pointer.id)
            .unwrap()
            .center()
    };
    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let val2 = app.eval_js("globalThis.__store.get(globalThis.__count).toString()");
    assert_eq!(
        val2, "55",
        "click must invoke the mutation against the instance store"
    );
}

/// `viewportSize$` is readable through the instance store — the public
/// handle reads its backing through the ENGINE read face (the ordinary
/// write rail), so reads resolve the same live value on every path, and
/// resizes propagate. No read path receives hidden values at `mount()`
/// time.
#[test]
fn engine_atoms_resolve_through_the_instance_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { mount, view, Text, viewportSize$ } from "tur:std";
        globalThis.__vp = viewportSize$;
        export function start({ store }) {
            globalThis.__store = store;
            mount(view(() => Text({ text: "x" })));
        }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let val = app.eval_js("globalThis.__store.get(globalThis.__vp).width.toString()");
    assert_eq!(
        val, "400",
        "viewportSize$ must be readable via the instance store"
    );

    // Resize propagates into the atom read through the store (the cached
    // handle copy goes stale; the next read recomputes).
    app.resize(500.0, 700.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let val2 = app.eval_js("globalThis.__store.get(globalThis.__vp).width.toString()");
    assert_eq!(
        val2, "500",
        "resize must update viewportSize$ read through the instance store"
    );
}

/// Engine atoms never depend on a mount: a module that never calls `mount`
/// still reads current engine truth through the instance store — before any
/// root exists and across resizes.
#[test]
fn engine_atom_readable_without_any_mount() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_module_raw(
        r#"
        import { viewportSize$ } from "tur:std";
        globalThis.__vp = viewportSize$;
        export function start({ store }) {
            globalThis.__s1 = store;
        }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Pre-mount read through the instance store.
    let before = app.eval_js("globalThis.__s1.get(globalThis.__vp).width.toString()");
    assert_eq!(
        before, "400",
        "viewportSize$ must resolve before any mount (engine backing, no root)"
    );

    // Resize with no root mounted at all — the write goes through the
    // engine rail, and the already-cached handle copy in s1 goes stale.
    app.resize(500.0, 700.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let after = app.eval_js("globalThis.__s1.get(globalThis.__vp).width.toString()");
    assert_eq!(
        after, "500",
        "resize must propagate into the store-cached handle copy with no root mounted"
    );
}

/// A declared derivation over an engine-minted atom keeps updating when the
/// engine atom changes — dependency edges through the instance store.
#[test]
fn declared_derive_over_engine_atom_updates_on_resize() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { derive, mount, view, Text, viewportSize$ } from "tur:std";
        const label = derive((ctx) => {
            const vp = ctx.get(viewportSize$);
            return vp.width + "x" + vp.height;
        });
        globalThis.__label = label;
        export function start({ store }) {
            globalThis.__store = store;
            mount(view(() => Text({ text: label })));
        }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let before = app.eval_js("globalThis.__store.get(globalThis.__label)");
    app.resize(500.0, 700.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let after = app.eval_js("globalThis.__store.get(globalThis.__label)");
    assert_eq!(before, "400x600");
    assert_eq!(after, "500x700");
}

/// `getStore` is not exported (import fails at link time) — embedded code
/// threads the closure ctx through instead (see `ctx_threads_the_instance_store`).
#[test]
fn get_store_is_not_exported() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    let get_store_gone = app
        .load_module_raw(r#"import { getStore } from "tur:std"; export function start() {}"#)
        .is_err();
    assert!(
        get_store_gone,
        "importing `getStore` from tur:std must fail at link time"
    );
}

/// The pattern that replaces `getStore()`: the `{get, set}` ctx handed to a
/// mutate closure is a stable store-bound reader/writer, so it can be
/// threaded into helper functions and captured by async fns — reads/writes
/// flow to the instance store without ever holding the store.
#[test]
fn ctx_threads_the_instance_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            mount, mutate, sleep, source, view, Text,
        } from "tur:std";
        const n = source(0);
        const ticks = source(0);
        globalThis.__n = n;

        // Helper takes ctx (never the store). The async fn captures it too.
        function runLoop(ctx, rounds) {
            (async () => {
                for (let i = 0; i < rounds; i++) {
                    await sleep(0).promise;
                    ctx.set(n, ctx.get(n) + 1);
                    ctx.set(ticks, i + 1);
                }
            })();
        }

        const startLoop = mutate((ctx, rounds) => runLoop(ctx, rounds));
        globalThis.__startLoop = startLoop;

        export function start({ store }) {
            globalThis.__store = store;
            mount(view(() => Text({ text: "loop" })));
        }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Invoke through the instance store (as an event dispatch would).
    app.eval_js("globalThis.__store.set(globalThis.__startLoop, 3); 'ok'");
    app.wait_for_timeout(std::time::Duration::ZERO);
    app.wait_for_timeout(std::time::Duration::from_millis(50));

    let val = app.eval_js("globalThis.__store.get(globalThis.__n).toString()");
    assert_eq!(
        val, "3",
        "ctx captured into an async fn must read/write the instance store"
    );
}

/// A derived that reads itself must not recurse natively (thread stack
/// overflow). The re-entrant read fails with a JS TypeError that PROPAGATES
/// to the read site (fail-fast derive semantics), and the atom stays stale —
/// it never materializes a sticky `undefined`.
#[test]
fn self_read_derive_fails_the_read_without_overflow() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { derive } from "tur:std";
        globalThis.__d = derive((ctx) => ctx.get(globalThis.__d));
        globalThis.__s = store;
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let val = app.eval_js(
        r#"(() => { try { return "ok:" + String(globalThis.__s.get(globalThis.__d)); }
                   catch (e) { return "threw:" + e.message; } })()"#,
    );
    assert_eq!(
        val, "threw:cycle detected: derived atom re-entered its own computation",
        "self-reading derive must fail the read with a TypeError, not overflow the stack"
    );
}

/// An indirect cycle through two deriveds (d1 -> d2 -> d1) is detected the
/// same way: the innermost re-entrant read errors and the TypeError
/// propagates out through both reads.
#[test]
fn indirect_derive_cycle_fails_the_read_without_overflow() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { derive } from "tur:std";
        globalThis.__d1 = derive((ctx) => ctx.get(globalThis.__d2));
        globalThis.__d2 = derive((ctx) => ctx.get(globalThis.__d1));
        globalThis.__s = store;
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let probe = r#"((h) => { try { return "ok:" + String(globalThis.__s.get(h)); }
                          catch (e) { return "threw:" + e.message; } })"#;
    let v1 = app.eval_js(&format!("{probe}(globalThis.__d1)"));
    let v2 = app.eval_js(&format!("{probe}(globalThis.__d2)"));
    assert_eq!(
        v1, "threw:cycle detected: derived atom re-entered its own computation",
        "indirect cycle d1 must fail the read, not overflow"
    );
    assert_eq!(
        v2, "threw:cycle detected: derived atom re-entered its own computation",
        "indirect cycle d2 must fail the read, not overflow"
    );
}
