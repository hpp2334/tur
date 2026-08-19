use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// `createStore()` returns a `{get, set}` store object; declarations carry no
/// state and are materialized per store. Same declaration in two stores =
/// two independent values.
#[test]
fn create_store_isolates_declaration_state_per_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { createStore, source } from "tur:std";
        globalThis.__n = source(0);
        globalThis.__s1 = createStore();
        globalThis.__s2 = createStore();
        globalThis.__s1.set(globalThis.__n, 1);
        globalThis.__s2.set(globalThis.__n, 2);
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let v1 = app.eval_js("globalThis.__s1.get(globalThis.__n).toString()");
    let v2 = app.eval_js("globalThis.__s2.get(globalThis.__n).toString()");
    assert_eq!(v1, "1", "s1 should hold its own materialized value");
    assert_eq!(
        v2, "2",
        "s2 should hold an independent value for the same decl"
    );
}

/// A store materializes a never-touched source declaration on first `get`,
/// seeding it with the declared initial value.
#[test]
fn store_get_materializes_with_initial_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { createStore, source } from "tur:std";
        globalThis.__s = createStore();
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

/// `mount(store, view)` mounts with the passed store; the free module-level
/// `get`/`set` exports are gone (import fails at link time) and `mount(store, view)`
/// without a store errors.
#[test]
fn mount_requires_store_and_free_get_set_are_gone() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    // Free `get`/`set` must not be exported from tur:std anymore.
    let free_fns_gone = app
        .load_module_raw(r#"import { get } from "tur:std"; export function start() {}"#)
        .is_err();
    assert!(
        free_fns_gone,
        "importing free `get` from tur:std must fail at link time"
    );
    let free_set_gone = app
        .load_module_raw(r#"import { set } from "tur:std"; export function start() {}"#)
        .is_err();
    assert!(
        free_set_gone,
        "importing free `set` from tur:std must fail at link time"
    );

    // mount(store, view) without a store must fail the module load (start throws).
    let bad_mount = app
        .load_module_raw(
            r#"
            import { mount, view, Text } from "tur:std";
            export function start() { mount(store, view(() => Text({ text: "x" }))); }
            "#,
        )
        .is_err();
    assert!(bad_mount, "mount(store, view) without a store must error");

    // mount(store, view) renders.
    app.eval_module_source(
        r#"
        import { createStore, mount, view, Text } from "tur:std";
        const store = createStore();
        export function start() {
            mount(store, view(() => Text({ text: "hello-store" })));
        }
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

/// Tree-driven declarations (props) materialize into the MOUNTED store: a
/// module-level `store.get` on the same declaration reads the same atom.
#[test]
fn prop_declarations_share_the_mounted_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { createStore, mount, mutate, source, view, PointerInteract, Text } from "tur:std";
        const store = createStore();
        globalThis.__store = store;
        const count = source(0);
        globalThis.__count = count;
        const bump = mutate((ctx) => ctx.set(count, 99));
        globalThis.__bump = bump;
        const clickBump = mutate((ctx) => ctx.set(count, 55));
        const ui = view(() => PointerInteract({
            onClick: clickBump,
            child: Text({ text: "click-me" }),
        }));
        export function start() { mount(store, ui); }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Invoke the same mutation decl the click uses, through the store —
    // verifies the decl materializes into the mounted store's KV.
    app.eval_js("globalThis.__store.set(globalThis.__bump); 'ok'");
    app.wait_for_timeout(std::time::Duration::ZERO);
    let val = app.eval_js("globalThis.__store.get(globalThis.__count).toString()");
    assert_eq!(
        val, "99",
        "store.set(mutation) must hit the mounted store's atom"
    );

    // Now the click path: the pointer event enqueues the same decl, which the
    // flush resolves against the mounted store.
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
        "click must invoke the mutation against the mounted store"
    );
}

/// Engine-minted atoms (e.g. `viewportSize$`) stay readable through a user
/// store — cross-store routing via the shared machinery.
#[test]
fn engine_atoms_route_through_user_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { createStore, viewportSize$ } from "tur:std";
        const store = createStore();
        globalThis.__store = store;
        globalThis.__vp = viewportSize$;
        export function start() {}
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    let val = app.eval_js("globalThis.__store.get(globalThis.__vp).width.toString()");
    assert_eq!(
        val, "400",
        "engine-minted viewportSize$ must route through a user store"
    );

    // Resize propagates into the atom read via the user store.
    app.resize(500.0, 700.0);
    app.wait_for_timeout(std::time::Duration::ZERO);
    let val2 = app.eval_js("globalThis.__store.get(globalThis.__vp).width.toString()");
    assert_eq!(
        val2, "500",
        "resize must update viewportSize$ read via user store"
    );
}

/// A declared derivation over an engine-minted atom (mounted store) keeps
/// updating when the engine atom changes — cross-store dependency edges.
#[test]
fn declared_derive_over_engine_atom_updates_on_resize() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import { createStore, derive, mount, view, Text, viewportSize$ } from "tur:std";
        const store = createStore();
        globalThis.__store = store;
        const label = derive((ctx) => {
            const vp = ctx.get(viewportSize$);
            return vp.width + "x" + vp.height;
        });
        globalThis.__label = label;
        export function start() {
            mount(store, view(() => Text({ text: label })));
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
/// threads the closure ctx through instead (see `ctx_threads_the_mounted_store`).
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
/// threaded into helper functions and captured by `launch` generators —
/// reads/writes flow to the mounted store without ever holding the store.
#[test]
fn ctx_threads_the_mounted_store() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(
        r#"
        import {
            createStore, launch, mount, mutate, sleep, source, view, Text,
        } from "tur:std";
        const store = createStore();
        const n = source(0);
        const ticks = source(0);
        globalThis.__n = n;
        globalThis.__store = store;

        // Helper takes ctx (never the store). The generator captures it too.
        function runLoop(ctx, rounds) {
            launch(function* () {
                for (let i = 0; i < rounds; i++) {
                    yield sleep(0);
                    ctx.set(n, ctx.get(n) + 1);
                    ctx.set(ticks, i + 1);
                }
            });
        }

        const startLoop = mutate((ctx, rounds) => runLoop(ctx, rounds));
        globalThis.__startLoop = startLoop;

        export function start() {
            mount(store, view(() => Text({ text: "loop" })));
        }
        "#,
    )
    .unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Invoke through the module's own store (as an event dispatch would).
    app.eval_js("globalThis.__store.set(globalThis.__startLoop, 3); 'ok'");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let val = app.eval_js("globalThis.__store.get(globalThis.__n).toString()");
    assert_eq!(
        val, "3",
        "ctx captured into a launch generator must read/write the mounted store"
    );
}
