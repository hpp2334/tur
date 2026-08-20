//! Multi-instance: one `TurRuntime` spawns multiple isolated `TurApp`
//! instances. Verifies JS-realm isolation (each instance has independent
//! global state), shared-runtime semantics (same fonts/clock/capabilities),
//! the plugin compile/register split, and independent event routing.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::capability::Capability;
use tur_engine::core::platform::PlatformEvent;
use tur_engine::core::plugin::{CompileContext, Plugin, PluginContext};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::renderer::NoopRenderer;
use tur_integration_tests::MutexFixedClock;
use tur_integration_tests::RawAppLooper;
use tur_integration_tests::TestSchedulerDriver;
use tur_native::NativeFontLoader;

/// Eval a JS script on a `TurApp` and return the result as a string.
/// Uses the engine's `eval_js` RPC (worker-thread JS evaluation).
/// If the result is a string, returns its contents (no quotes);
/// otherwise returns the display form.
fn eval_js(app: &Rc<tur_engine::TurApp>, source: &str) -> Result<String, String> {
    Ok(futures::executor::block_on(app.backend().eval_js(source)))
}

/// Build a runtime with the std + animation plugins (no extra capabilities —
/// instances are headless). Returns the driver too so callers can spawn a
/// `RawAppLooper` per instance, plus the (effectively uncapped) default
/// worker pool each instance assigns.
fn build_runtime() -> (Rc<TurRuntime>, Rc<TestSchedulerDriver>, WorkerPoolHandle) {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("test", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .vsync_source(driver.vsync_source())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .build()
        .expect("runtime build");
    (runtime, driver, pool)
}

const SET_ID_JS: &str = r#"const store = createStore();

    import { createStore, Text, mount } from "tur:std";
    export function start() {
        globalThis.__instanceId = "VALUE";
        mount(store, Text({ text: "VALUE" }));
    }
"#;

#[test]
fn instances_have_isolated_js_realms() {
    let (runtime, _driver, pool) = build_runtime();
    let app_a = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("app A");
    let app_b = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("app B");

    // Load different state into each instance.
    futures::executor::block_on(
        app_a
            .backend()
            .load_module(SET_ID_JS.replace("VALUE", "A").as_str()),
    )
    .expect("load A");
    futures::executor::block_on(
        app_b
            .backend()
            .load_module(SET_ID_JS.replace("VALUE", "B").as_str()),
    )
    .expect("load B");

    // Each instance reads back its OWN global — they must differ.
    let id_a = eval_js(&app_a, "globalThis.__instanceId").expect("eval A");
    let id_b = eval_js(&app_b, "globalThis.__instanceId").expect("eval B");
    assert_eq!(id_a, "A", "instance A should have its own state");
    assert_eq!(id_b, "B", "instance B should have its own state");

    // Mutating A must not affect B.
    eval_js(&app_a, r#"globalThis.__instanceId = "A2""#).unwrap();
    let id_b_after = eval_js(&app_b, "globalThis.__instanceId").unwrap();
    assert_eq!(id_b_after, "B", "instance B unaffected by A's mutation");
}

#[test]
fn instances_have_isolated_element_trees() {
    let (runtime, driver, pool) = build_runtime();
    let app_a = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("app A");
    let app_b = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("app B");
    let looper_a = RawAppLooper::new(app_a.clone(), driver.clone());

    // Mount a tree only in A.
    futures::executor::block_on(app_a.backend().load_module(
        r#"const store = createStore();

            import { createStore, Text, mount } from "tur:std";
            export function start() {
                mount(store, Text({ text: "only-in-A", queryKey: ["a_only"] }));
            }
        "#,
    ))
    .expect("load A");
    looper_a.wait_for_timeout(Duration::ZERO);

    // B has no tree mounted.
    let b_tree = futures::executor::block_on(app_b.with_tree(|tree, _focus| {
        tree.root_element_id()
            .and_then(|root| tree.dev_tool_node(root.into()))
    }))
    .flatten();
    assert!(b_tree.is_none(), "instance B should have no tree");

    // A does have a tree.
    let a_tree = futures::executor::block_on(app_a.with_tree(|tree, _focus| {
        tree.root_element_id()
            .and_then(|root| tree.dev_tool_node(root.into()))
    }))
    .flatten();
    assert!(a_tree.is_some(), "instance A should have a tree");
}

#[test]
fn headless_instance_runs_js_without_rendering() {
    let (runtime, driver, pool) = build_runtime();
    let app = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (0.0, 0.0), 1.0)
        .build()
        .expect("headless");
    let looper = RawAppLooper::new(app.clone(), driver);

    // JS executes; a frame runs without panic even with a zero viewport.
    futures::executor::block_on(app.backend().load_module(
        r#"const store = createStore();

        import { createStore, source } from "tur:std";
        export function start() {
            globalThis.__val = source(42);
            const v = store.get(globalThis.__val);
            globalThis.__readBack = v;
        }
    "#,
    ))
    .expect("load");
    looper.wait_for_timeout(Duration::ZERO);

    let val = eval_js(&app, "globalThis.__readBack").expect("eval");
    assert_eq!(val, "42", "headless instance ran JS");
}

/// `TurAppBuilder::build_headless` is the dedicated headless entry point
/// (no render target). Unlike the pre-threading inline headless path, it
/// must run the engine on a worker — i.e. JS execution round-trips through
/// the worker pipeline (load_module / pump / eval_js are all RPCs that
/// cross main↔worker). This test pins both the API surface and that the
/// worker is actually driving the instance.
#[test]
fn build_headless_runs_engine_on_worker() {
    let (runtime, driver, pool) = build_runtime();
    let app = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .build_headless((0.0, 0.0))
        .expect("headless_app");
    let looper = RawAppLooper::new(app.clone(), driver);

    // JS executes via the worker RPC path.
    futures::executor::block_on(app.backend().load_module(
        r#"const store = createStore();

        import { createStore, source } from "tur:std";
        export function start() {
            globalThis.__val = source(7);
            globalThis.__readBack = store.get(globalThis.__val);
        }
    "#,
    ))
    .expect("load");
    looper.wait_for_timeout(Duration::ZERO);

    let val = eval_js(&app, "globalThis.__readBack").expect("eval");
    assert_eq!(val, "7", "build_headless ran JS on the worker");
}

#[test]
fn many_instances_share_one_runtime() {
    // Smoke test: spawn several instances from one runtime to confirm no
    // shared-state corruption (each gets its own boa Context + store).
    let (runtime, _driver, pool) = build_runtime();
    let mut apps = Vec::new();
    for i in 0..5 {
        let app = runtime
            .app_builder()
            .worker_pool(pool.clone())
            .renderer(Box::new(NoopRenderer::new()), (50.0, 50.0), 1.0)
            .build()
            .expect("app");
        // No `start` ceremony needed for pure state — `eval_js` runs a
        // classic script in the same realm.
        eval_js(&app, &format!(r#"globalThis.__idx = {i};"#)).expect("load");
        apps.push(app);
    }
    for (i, app) in apps.iter().enumerate() {
        let idx = eval_js(app, "globalThis.__idx").expect("eval");
        assert_eq!(idx, i.to_string(), "instance {i} should have its own __idx");
    }
}

// === Shared-capability + plugin compile/register split =====================

/// A test-only capability carrying a shared counter.
#[derive(Clone)]
struct SharedCounterCap {
    value: Arc<AtomicU32>,
}
impl Capability for SharedCounterCap {}
impl SharedCounterCap {
    fn new() -> Self {
        Self {
            value: Arc::new(AtomicU32::new(0)),
        }
    }
    fn get(&self) -> u32 {
        self.value.load(Ordering::SeqCst)
    }
    fn bump(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }
}

/// Plugin that, in `register`, looks up the shared `SharedCounterCap` from the
/// capability registry and bumps it. Counts `compile` vs `register` invocations
/// to pin the compile/register split.
struct CounterPlugin {
    compile_count: Arc<AtomicU32>,
    register_count: Arc<AtomicU32>,
}

impl Plugin for CounterPlugin {
    fn compile(&self, _cx: &mut CompileContext) -> Result<(), tur_engine::error::TurError> {
        self.compile_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), tur_engine::error::TurError> {
        self.register_count.fetch_add(1, Ordering::SeqCst);
        // Look up the shared capability (registered on the runtime builder) and
        // bump it — proves every instance's register sees the SAME backend.
        if let Some(cap) = ctx.capability().of::<SharedCounterCap>() {
            cap.bump();
        }
        Ok(())
    }
}

#[test]
fn plugin_compile_runs_once_register_runs_per_instance() {
    let compile_count = Arc::new(AtomicU32::new(0));
    let register_count = Arc::new(AtomicU32::new(0));
    let pool = WorkerPoolHandle::new("test", usize::MAX);
    let driver = tur_integration_tests::TestSchedulerDriver::new();
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .vsync_source(driver.vsync_source())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .plugin(CounterPlugin {
            compile_count: compile_count.clone(),
            register_count: register_count.clone(),
        })
        .build()
        .expect("runtime");

    // compile ran exactly once at build time.
    assert_eq!(
        compile_count.load(Ordering::SeqCst),
        1,
        "compile runs once on the runtime"
    );
    // register has NOT run yet (no instances created).
    assert_eq!(
        register_count.load(Ordering::SeqCst),
        0,
        "register runs per instance, not at build"
    );

    // Spawn 3 instances — register fires once each, compile stays at 1.
    for _ in 0..3 {
        runtime
            .app_builder()
            .worker_pool(pool.clone())
            .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
            .build()
            .expect("app");
    }
    assert_eq!(
        compile_count.load(Ordering::SeqCst),
        1,
        "compile never re-runs per instance"
    );
    assert_eq!(
        register_count.load(Ordering::SeqCst),
        3,
        "register ran once per spawned instance"
    );
}

#[test]
fn shared_capability_backend_is_visible_from_all_instances() {
    let compile_count = Arc::new(AtomicU32::new(0));
    let register_count = Arc::new(AtomicU32::new(0));
    let cap = SharedCounterCap::new();
    let pool = WorkerPoolHandle::new("test", usize::MAX);
    let driver = tur_integration_tests::TestSchedulerDriver::new();
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .vsync_source(driver.vsync_source())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .capability({
            let c = cap.clone();
            move |_| Ok(c)
        })
        .plugin(TurStdPlugin)
        .plugin(CounterPlugin {
            compile_count: compile_count.clone(),
            register_count: register_count.clone(),
        })
        .build()
        .expect("runtime");

    // Each spawned instance's `register` bumps the SAME shared cap — so after
    // N instances the cap reflects N bumps, proving they all see one backend.
    runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
        .build()
        .expect("A");
    assert_eq!(cap.get(), 1, "instance A's register bumped the shared cap");
    runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
        .build()
        .expect("B");
    assert_eq!(
        cap.get(),
        2,
        "instance B's register saw the same shared cap"
    );
    runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
        .build()
        .expect("C");
    assert_eq!(
        cap.get(),
        3,
        "instance C's register saw the same shared cap"
    );
}

// === Independent event routing + reactive isolation =======================

#[test]
fn platform_events_route_to_the_correct_instance() {
    let (runtime, driver, pool) = build_runtime();
    let app_a = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("A");
    let app_b = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("B");
    let looper_a = RawAppLooper::new(app_a.clone(), driver.clone());
    let looper_b = RawAppLooper::new(app_b.clone(), driver);

    // Push a Resize to A only.
    app_a.push_platform_event(PlatformEvent::Resize {
        logical_width: 250,
        logical_height: 180,
        dpr: 1.0,
    });
    looper_a.wait_for_timeout(Duration::ZERO);
    looper_b.wait_for_timeout(Duration::ZERO);

    // Read back each instance's viewportSize$ via JS. `eval_js` runs in script
    // mode (no imports), so do the import in a module eval and stash the JSON
    // on globalThis, then read it back with eval_js.
    let read_vp = |app: &Rc<tur_engine::TurApp>| -> String {
        let _ = futures::executor::block_on(app.backend().load_module(
            r#"import { createStore, viewportSize$ } from "tur:std";
const store = createStore();

               export function start() {
                   globalThis.__vp = JSON.stringify(store.get(viewportSize$));
               }"#,
        ));
        eval_js(app, "globalThis.__vp").unwrap_or_default()
    };

    let a_vp = read_vp(&app_a);
    let b_vp = read_vp(&app_b);

    assert!(
        a_vp.contains("250"),
        "instance A viewport resized to 250: {a_vp}"
    );
    assert!(
        b_vp.contains("100") && !b_vp.contains("250"),
        "instance B viewport unaffected by A's resize: {b_vp}"
    );
}

#[test]
fn reactive_stores_are_isolated_per_instance() {
    let (runtime, _driver, pool) = build_runtime();
    let app_a = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("A");
    let app_b = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("B");

    // Create a source in A and set a value. The store is stashed on
    // globalThis so the read-back module below reads through the SAME store
    // (a fresh store would materialize the declaration independently).
    futures::executor::block_on(app_a.backend().load_module(
        r#"import { createStore, source } from "tur:std";
           const store = createStore();
           globalThis.__store = store;
           export function start() {
               globalThis.__atom = source("from-A");
               store.set(globalThis.__atom, "A2");
           }"#,
    ))
    .expect("setup A");

    // B has no such atom — `globalThis.__atom` is undefined in B's realm.
    let b_val = eval_js(
        &app_b,
        "typeof globalThis.__atom === 'undefined' ? 'none' : 'present'",
    )
    .expect("eval B");
    assert_eq!(
        b_val, "none",
        "instance B should not see instance A's reactive atoms"
    );

    // A still has its own value. `import` needs module context, so eval as a
    // module and stash on globalThis, then read back.
    futures::executor::block_on(app_a.backend().load_module(
        r#"export function start() {
               globalThis.__r = globalThis.__store.get(globalThis.__atom);
           }"#,
    ))
    .expect("read A");
    let a_val = eval_js(&app_a, "globalThis.__r").unwrap_or_default();
    assert_eq!(a_val, "A2", "instance A retains its own reactive state");
}
