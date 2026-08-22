//! Plugin-facing reactive substrate: plugins can mint sources / derives /
//! mutations from Rust (via `PluginContext::reactive()` →
//! `ReactiveBridgeStore`) and expose them to JS. JS reads/writes the atoms
//! through the unchanged `tur:core` / `tur:std` bridge (`get` / `set`), so
//! the JS side cannot tell whether an atom was minted by Rust or JS.
//!
//! The Rust-native `build_derive` / `build_mutate` variants skip the
//! `{get, set}` JsObject round-trip — the closure receives a typed
//! capability face directly.

use std::time::Duration;

use boa_engine::{JsArgs, JsValue};
use tur_engine::core::edgy::reactive::{ReactiveBridgeStore, Readable, Source};
use tur_engine::core::js_runtime::js_value::IntoJs;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::subsystem::{Subsystem, SubsystemFlushContext};
use tur_engine::error::TurError;
use tur_integration_tests::TurTestApp;

// ---------------------------------------------------------------------------
// source(): plugin mints a source from Rust, exposes as a JS global
// ---------------------------------------------------------------------------

struct MintSourcePlugin;
impl Plugin for MintSourcePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let bridge = ctx.reactive();
        let s: Source<JsValue> = bridge.source(JsValue::new(42.0));
        let js_handle = s.into_js(ctx.boa_mut());
        ctx.register_global("rustSource", js_handle);
        Ok(())
    }
}

/// `bridge.source(v)` mints a source whose value is readable from JS via
/// the unchanged `get(source)` bridge.
#[test]
fn plugin_can_mint_source_readable_from_js() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(MintSourcePlugin)])
        .expect("app build");
    app.eval_module_source(
        r#"import { createStore } from "tur:std";
const store = createStore();
            globalThis.__v = store.get(globalThis.rustSource);
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);

    let v = app.eval_js("globalThis.__v");
    assert_eq!(v, "42", "JS should read the Rust-minted source via get()");
}

/// `set(rustSource, v)` from JS routes through the bridge's `set_source`,
/// symmetric with the JS-minted-source case.
#[test]
fn plugin_minted_source_is_writable_from_js_via_set() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(MintSourcePlugin)])
        .expect("app build");
    app.eval_module_source(
        r#"const store = createStore();

        import { createStore } from "tur:std";
        store.set(globalThis.rustSource, 99);
        globalThis.__v = store.get(globalThis.rustSource);
        "#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);

    let v = app.eval_js("globalThis.__v");
    assert_eq!(
        v, "99",
        "JS should write+read the Rust-minted source via set()/get()"
    );
}

// ---------------------------------------------------------------------------
// build_derive(): plugin computes a derived value via a Rust closure
// ---------------------------------------------------------------------------

struct BuildDerivePlugin;
impl Plugin for BuildDerivePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let bridge = ctx.reactive();
        let a: Source<JsValue> = bridge.source(JsValue::new(10.0));
        let b: Source<JsValue> = bridge.source(JsValue::new(20.0));

        // Rust-native derive that reads both sources. Reads flow through the
        // same `ReactiveCore::read` path as JS closures, so auto-dependency
        // tracking is inherited for free.
        let sum = bridge.build_derive(move |read, boa| {
            let av = read.read(Readable::from(a), boa).as_number().unwrap_or(0.0);
            let bv = read.read(Readable::from(b), boa).as_number().unwrap_or(0.0);
            Ok(JsValue::new(av + bv))
        });

        // Build all JS handles under a single `boa_mut()` borrow scope.
        let (a_js, b_js, sum_js) = {
            let boa = ctx.boa_mut();
            (a.into_js(boa), b.into_js(boa), sum.into_js(boa))
        };
        ctx.register_global("a$", a_js);
        ctx.register_global("b$", b_js);
        ctx.register_global("sum$", sum_js);
        Ok(())
    }
}

/// `build_derive(|read, ctx| ...)` produces a `Derived<JsValue>` whose
/// recompute runs the Rust closure (no `{get, set}` JsObject round-trip).
#[test]
fn plugin_build_derive_recomputes_via_rust_closure() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(BuildDerivePlugin)])
        .expect("app build");
    app.eval_module_source(
        r#"import { createStore } from "tur:std";
const store = createStore();
            globalThis.__sum = store.get(globalThis.sum$);
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.eval_js("globalThis.__sum"),
        "30",
        "initial sum should be 10 + 20"
    );

    // Update one source; the derive must recompute lazily on the next read.
    app.eval_module_source(
        r#"const store = createStore();

        import { createStore } from "tur:std";
        store.set(globalThis.a$, 100);
        globalThis.__sum = store.get(globalThis.sum$);
        "#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.eval_js("globalThis.__sum"),
        "120",
        "after set(a$, 100) the derive should recompute to 100 + 20"
    );
}

/// After a `set` to one of the deps, the dirty propagation marks the derived
/// stale; the next `get` triggers recompute. The Rust closure reads through
/// `ReactiveReadStore::read`, so the `tracker_stack` records deps exactly
/// like the JS path — this test pins that the auto-dep path is wired for
/// the Rust variant too.
#[test]
fn plugin_build_derive_dirty_propagation_across_multiple_updates() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(BuildDerivePlugin)])
        .expect("app build");

    for (set_a, set_b, expected) in [(5.0, 5.0, 10.0), (50.0, 50.0, 100.0), (-1.0, 1.0, 0.0)] {
        app.eval_module_source(&format!(
            r#"import {{ createStore }} from "tur:std";
const store = createStore();
            store.set(globalThis.a$, {set_a});
            store.set(globalThis.b$, {set_b});
            globalThis.__sum = store.get(globalThis.sum$);
            "#,
        ))
        .expect("eval");
        app.wait_for_timeout(Duration::ZERO);
        let got = app.eval_js("globalThis.__sum");
        assert_eq!(
            got.parse::<f64>().unwrap_or(f64::NAN),
            expected,
            "after set(a$={set_a}, b$={set_b}) the derive should recompute to {expected}"
        );
    }
}

// ---------------------------------------------------------------------------
// build_mutate(): plugin encapsulates a state transition in a Rust closure
// ---------------------------------------------------------------------------

struct BuildMutatePlugin;
impl Plugin for BuildMutatePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let bridge = ctx.reactive();
        let flag: Source<JsValue> = bridge.source(JsValue::new(false));

        // Capture clones for the closure: `flag` is `Copy`, `bridge` is
        // `Clone` (cheap Rc bump). The closure runs on the worker during
        // `invoke_mutation`, so neither needs to be `Send`.
        let flag_for_closure = flag;
        let bridge_for_closure = bridge.clone();
        let toggle = bridge.build_mutate(move |b, _args, boa| {
            let current = b
                .read(Readable::from(flag_for_closure), boa)
                .as_boolean()
                .unwrap_or(false);
            bridge_for_closure.set_source(flag_for_closure, JsValue::new(!current));
            Ok(JsValue::undefined())
        });

        let (flag_js, toggle_js) = {
            let boa = ctx.boa_mut();
            (flag.into_js(boa), toggle.into_js(boa))
        };
        ctx.register_global("flag$", flag_js);
        ctx.register_global("toggle", toggle_js);
        Ok(())
    }
}

/// `set(mutation)` from JS routes through `invoke_mutation`, which detects
/// the `MutateRust` variant and calls the closure with the bridge face +
/// user args (no `{get, set}` JsObject prepended).
#[test]
fn plugin_build_mutate_runs_rust_closure_on_js_set() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(BuildMutatePlugin)])
        .expect("app build");

    let read_flag = |app: &TurTestApp| -> bool {
        app.eval_module_source(
            r#"import { createStore } from "tur:std";
const store = createStore();
globalThis.__f = store.get(globalThis.flag$);
"#,
        )
        .expect("eval");
        app.wait_for_timeout(Duration::ZERO);
        app.eval_js("globalThis.__f") == "true"
    };

    assert!(!read_flag(&app), "flag$ starts false");

    app.eval_module_source(
        r#"import { createStore } from "tur:std";
const store = createStore();
store.set(globalThis.toggle);
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        read_flag(&app),
        "flag$ should flip to true after one toggle"
    );

    app.eval_module_source(
        r#"import { createStore } from "tur:std";
const store = createStore();
store.set(globalThis.toggle);
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        !read_flag(&app),
        "flag$ should flip back to false after a second toggle"
    );
}

/// `build_mutate` closures receive user args verbatim (no JsObject prepended).
struct BuildMutateWithArgsPlugin;
impl Plugin for BuildMutateWithArgsPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let bridge = ctx.reactive();
        let sink: Source<JsValue> = bridge.source(JsValue::undefined());

        let sink_for_closure = sink;
        let bridge_for_closure = bridge.clone();
        let write_msg = bridge.build_mutate(move |_b, args, _boa| {
            let arg = args.get_or_undefined(0).clone();
            bridge_for_closure.set_source(sink_for_closure, arg);
            Ok(JsValue::undefined())
        });

        let (sink_js, write_msg_js) = {
            let boa = ctx.boa_mut();
            (sink.into_js(boa), write_msg.into_js(boa))
        };
        ctx.register_global("sink$", sink_js);
        ctx.register_global("writeMsg", write_msg_js);
        Ok(())
    }
}

#[test]
fn plugin_build_mutate_receives_user_args_verbatim() {
    let app =
        TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(BuildMutateWithArgsPlugin)])
            .expect("app build");

    app.eval_module_source(
        r#"const store = createStore();

        import { createStore } from "tur:std";
        store.set(globalThis.writeMsg, "hello", "ignored-extra");
        globalThis.__v = store.get(globalThis.sink$);
        "#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);

    let v = app.eval_js("globalThis.__v");
    assert_eq!(
        v, "hello",
        "build_mutate closure should receive user args[0] verbatim (no JsObject prepended)"
    );
}

// ---------------------------------------------------------------------------
// Subsystem + Rust-minted source: subsystem writes via set_source, JS observes
// ---------------------------------------------------------------------------

struct CounterSubsystem {
    source: Source<JsValue>,
    bridge: ReactiveBridgeStore,
    tick: u32,
}

impl Subsystem for CounterSubsystem {
    fn flush_pre_layout(&mut self, _cx: &mut SubsystemFlushContext<'_>) {
        // Cap to avoid runaway growth; the test only needs to observe that
        // at least one bump made it through.
        if self.tick >= 5 {
            return;
        }
        self.tick += 1;
        self.bridge
            .set_source(self.source, JsValue::new(self.tick as f64));
    }
}

struct SubsystemTickPlugin;
impl Plugin for SubsystemTickPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let bridge = ctx.reactive();
        let counter: Source<JsValue> = bridge.source(JsValue::new(0.0));
        let counter_js = counter.into_js(ctx.boa_mut());
        ctx.register_global("counter$", counter_js);
        ctx.register_subsystem(Box::new(CounterSubsystem {
            source: counter,
            bridge,
            tick: 0,
        }));
        Ok(())
    }
}

/// A plugin's subsystem can write to a Rust-minted source each frame via
/// `bridge.set_source(...)`, and JS observes the updated value. Mirrors the
/// engine-internal `viewportSize$` pattern (engine-owned source updated by
/// `Screen::sync_source` on resize).
#[test]
fn plugin_subsystem_writes_to_minted_source_observable_from_js() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(SubsystemTickPlugin)])
        .expect("app build");

    // Drive a few frames so flush_pre_layout ticks at least once.
    app.wait_for_timeout(Duration::from_millis(64));

    app.eval_module_source(
        r#"import { createStore } from "tur:std";
const store = createStore();
globalThis.__c = store.get(globalThis.counter$);
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);

    let v: u32 = app.eval_js("globalThis.__c").parse().unwrap_or(0);
    assert!(
        v > 0,
        "subsystem should have bumped counter$ at least once across driven frames (got {v})"
    );
}

// ---------------------------------------------------------------------------
// cycle guard: a Rust-native derive reading itself must not overflow the
// thread stack. There are no JS frames on this path, so the engine (not
// boa's runtime limits) must detect the re-entrant compute.
// ---------------------------------------------------------------------------

struct SelfReadDerivePlugin;
impl Plugin for SelfReadDerivePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        use std::cell::RefCell;
        use std::rc::Rc;
        use tur_engine::core::edgy::reactive::Derived;

        let bridge = ctx.reactive();
        // Chicken-and-egg: the closure needs its own handle, so we hand it a
        // cell filled immediately after `build_derive` returns.
        let handle: Rc<RefCell<Option<Derived<JsValue>>>> = Rc::new(RefCell::new(None));
        let handle_for_closure = handle.clone();
        let d = bridge.build_derive(move |read, boa| {
            let Some(d) = handle_for_closure.borrow().clone() else {
                return Ok(JsValue::undefined());
            };
            Ok(read.read(Readable::from(d), boa))
        });
        *handle.borrow_mut() = Some(d);

        let d_js = d.into_js(ctx.boa_mut());
        ctx.register_global("cycle$", d_js);
        Ok(())
    }
}

#[test]
fn rust_derive_self_read_materializes_undefined_without_overflow() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(SelfReadDerivePlugin)])
        .expect("app build");
    app.eval_module_source(
        r#"import { createStore } from "tur:std";
const store = createStore();
            globalThis.__v = store.get(globalThis.cycle$);
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        app.eval_js("String(globalThis.__v)"),
        "undefined",
        "self-reading Rust derive must materialize undefined, not overflow the stack"
    );
}
