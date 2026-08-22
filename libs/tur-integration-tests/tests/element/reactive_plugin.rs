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
        let s: Source<JsValue> = bridge.decl_source(JsValue::new(42.0));
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
        r#"globalThis.__v = store.get(globalThis.rustSource);
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
        r#"
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
        let a: Source<JsValue> = bridge.decl_source(JsValue::new(10.0));
        let b: Source<JsValue> = bridge.decl_source(JsValue::new(20.0));

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
        r#"globalThis.__sum = store.get(globalThis.sum$);
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
        r#"
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
            r#"store.set(globalThis.a$, {set_a});
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
        let flag: Source<JsValue> = bridge.decl_source(JsValue::new(false));

        // The closure writes through the bridge face it RECEIVES (`b`), which
        // is bound to the invoking store — so reads/writes land in the same
        // store JS invoked the mutation through. Capturing the register-time
        // bridge instead would pin the engine store, and per-store
        // materialization means JS would never see the write.
        let toggle = bridge.build_mutate(move |b, _args, boa| {
            let current = b
                .read(Readable::from(flag), boa)
                .as_boolean()
                .unwrap_or(false);
            b.set_source(flag, JsValue::new(!current))?;
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
/// the MutateRust variant and calls the closure with the bridge face +
/// user args (no `{get, set}` JsObject prepended). The instance store is
/// stashed on globalThis and reused across evals — per-eval fresh state
/// would read the seed instead of the flip.
#[test]
fn plugin_build_mutate_runs_rust_closure_on_js_set() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(BuildMutatePlugin)])
        .expect("app build");

    // Stash the instance store for the toggles below.
    app.eval_module_source("globalThis.__store = store;");
    app.wait_for_timeout(Duration::ZERO);

    let read_flag = |app: &TurTestApp| -> bool {
        app.eval_js("String(globalThis.__store.get(globalThis.flag$))") == "true"
    };
    let toggle = |app: &TurTestApp| {
        app.eval_js("globalThis.__store.set(globalThis.toggle)");
        app.wait_for_timeout(Duration::ZERO);
    };

    assert!(!read_flag(&app), "flag$ starts false");
    toggle(&app);
    assert!(
        read_flag(&app),
        "flag$ should flip to true after one toggle"
    );
    toggle(&app);
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
        let sink: Source<JsValue> = bridge.decl_source(JsValue::undefined());

        // Write through the received face (the invoking store) — see
        // BuildMutatePlugin for why the register-time bridge must not be
        // captured.
        let write_msg = bridge.build_mutate(move |b, args, _boa| {
            let arg = args.get_or_undefined(0).clone();
            b.set_source(sink, arg)?;
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
        r#"store.set(globalThis.writeMsg, "hello", "ignored-extra");
        globalThis.__v = String(store.get(globalThis.sink$));
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
    last_frame: u64,
    tick: u32,
}

impl Subsystem for CounterSubsystem {
    fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        // Self-gate: at most one tick per frame (the canonical subsystem
        // pattern — an ungated write would dirty the app on every fixed-point
        // iteration and the flush loop would never reach quiescence).
        if self.last_frame == cx.frame_id {
            return;
        }
        self.last_frame = cx.frame_id;
        self.tick += 1;
        // Engine-atom pattern: the backing's one home is the ENGINE store,
        // so the write goes through the ordinary `set_source` rail via the
        // bridge captured at registration — no tree chase, works pre- and
        // post-mount. Readers reach the value through the exposed handle
        // (a derive whose closure reads the backing via the engine face),
        // exactly like `viewportSize$`.
        self.bridge
            .set_source(self.source, JsValue::new(self.tick as f64))
            .ok();
    }
}

struct SubsystemTickPlugin;
impl Plugin for SubsystemTickPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let bridge = ctx.reactive();
        let counter: Source<JsValue> = bridge.decl_source(JsValue::new(0.0));
        // The public handle: a derive reading the backing through the
        // ENGINE store's read face (captured), not the reading store's —
        // so every store of the instance resolves the same live value.
        let engine_read = bridge.read_only();
        let handle = bridge
            .build_derive(move |_read, boa| Ok(engine_read.read(Readable::from(counter), boa)));
        let counter_js = handle.into_js(ctx.boa_mut());
        ctx.register_global("counter$", counter_js);
        ctx.register_subsystem(Box::new(CounterSubsystem {
            source: counter,
            bridge,
            last_frame: 0,
            tick: 0,
        }));
        Ok(())
    }
}

/// A plugin's subsystem can publish to an engine atom each frame via the
/// ordinary write rail (`set_source` through the engine store), and JS
/// observes the updated value through ANY store — the canonical
/// engine-atom pattern shared with `viewportSize$`.
#[test]
fn plugin_subsystem_writes_to_minted_source_observable_from_js() {
    let app = TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(SubsystemTickPlugin)])
        .expect("app build");

    app.eval_module_source(
        r#"import { mount, view, Text } from "tur:std";
export function start({ store }) {
    globalThis.__store = store;
    mount(view(() => Text({ text: "" })));
}
"#,
    )
    .expect("eval");

    // Drive a few frames so flush_pre_layout ticks at least once.
    app.wait_for_timeout(Duration::from_millis(64));

    let v: u32 = app
        .eval_js("String(globalThis.__store.get(globalThis.counter$))")
        .parse()
        .unwrap_or(0);
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

/// A Rust-native derive reading itself must not recurse natively. The cycle
/// guard stops the recursion at the re-entrant `read_by_id`; the read
/// surfaces as `undefined` through the internal Rust face (the swallowing
/// fallback layout reads rely on — only JS closures propagate their errors),
/// so the derive materializes `undefined` without overflowing.
#[test]
fn rust_derive_self_read_materializes_undefined_without_overflow() {
    let app =
        TurTestApp::new_with_extra_plugins(200.0, 100.0, vec![Box::new(SelfReadDerivePlugin)])
            .expect("app build");
    app.eval_module_source(
        r#"globalThis.__v = String(store.get(globalThis.cycle$));
"#,
    )
    .expect("eval");
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        app.eval_js("globalThis.__v"),
        "undefined",
        "self-reading Rust derive must materialize undefined, not overflow the stack"
    );
}
