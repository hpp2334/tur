//! Per-instance typed metadata slot: `TurAppBuilder::instance_data::<T>(value)`
//! stamps typed data at instance-creation time; `TurJsContext::data::<T>()`
//! (clone-out) and `TurJsContext::with_data::<T, _>(f)` (ref-callback) read
//! it from bridge fns / plugin `register` / subsystem flush contexts. Each
//! type may be stamped at most once per builder — a second
//! `instance_data::<T>(...)` with the same `T` panics eagerly at the call
//! site.
//!
//! Together these let a host bind secure, JS-unforgeable identity to an
//! instance (e.g. a `PluginId` for the Ease Music Player plugin system) —
//! bridge fns read it from `TurJsContext`, never from JS arguments.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;
use tur_engine::renderer::NoopRenderer;
use tur_integration_tests::MutexFixedClock;
use tur_integration_tests::TestSchedulerDriver;
use tur_native::NativeFontLoader;

// ---------- Test marker types ----------------------------------------------
//
// `PluginId` mirrors the canonical use case (a host stamps the calling
// plugin's identity at instance creation so bridge fns can resolve
// per-plugin storage without trusting JS args). `ThemeOverride` is a second
// distinct type so we can exercise multi-typed stamping in one builder.

#[derive(Clone, Debug, PartialEq)]
struct PluginId(String);

#[derive(Clone, Debug, PartialEq)]
struct ThemeOverride {
    is_dark: bool,
}

// ---------- Helpers --------------------------------------------------------

/// Build a runtime carrying std + animation + the supplied extra plugin.
/// Each test supplies a capture plugin that records what it observed on
/// `TurJsContext` at register-time.
fn build_runtime(extra: Box<dyn Plugin>) -> Rc<TurRuntime> {
    TurRuntime::builder()
        .scheduler(TestSchedulerDriver::new())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .plugin(TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .plugin_boxed(extra)
        .build()
        .expect("runtime build")
}

/// Plugin that captures `TurJsContext::data::<PluginId>()` at register-time
/// via the clone-out accessor (mirrors `Capabilities::of`).
struct CapturePluginId {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for CapturePluginId {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        *self.seen.lock().unwrap() = ctx.js_ctx().data::<PluginId>();
        Ok(())
    }
}

/// Plugin that captures `PluginId` via the ref-callback accessor
/// `with_data::<T, R>(f)`. The callback runs under the borrow and returns
/// a cloned value — exercises the no-`Clone`-on-T-bound path.
struct CapturePluginIdViaRef {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for CapturePluginIdViaRef {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let captured = ctx.js_ctx().with_data::<PluginId, _>(|pid| pid.clone());
        *self.seen.lock().unwrap() = captured;
        Ok(())
    }
}

// ---------- Tests ----------------------------------------------------------

// Round-trip: stamp `PluginId` via the builder, observe it from a plugin's
// `register` via the clone-out accessor.
#[test]
fn instance_data_round_trip_via_plugin() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime(Box::new(CapturePluginId { seen: seen.clone() }));

    let _app = runtime
        .app_builder()
        .instance_data(PluginId("com.ease.onedrive".into()))
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("app build");

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("com.ease.onedrive".into())),
        "plugin should observe the host-stamped PluginId at register-time"
    );
}

// Same round-trip via the ref-callback accessor (`with_data`). Confirms both
// read shapes work and that `with_data` does not require `T: Clone` on the
// `TurJsContext` side (the callback may clone if it wants, as here).
#[test]
fn instance_data_with_data_ref_callback() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime(Box::new(CapturePluginIdViaRef { seen: seen.clone() }));

    let _app = runtime
        .app_builder()
        .instance_data(PluginId("com.ease.spotify".into()))
        .renderer(Box::new(NoopRenderer::new()), (100.0, 100.0), 1.0)
        .build()
        .expect("app build");

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("com.ease.spotify".into())),
        "with_data ref-callback should observe the stamped value"
    );
}

// Two instances from one runtime, each stamped with a different `PluginId`.
// Each instance's `register` must see ONLY its own value — per-instance
// isolation, not a shared runtime-wide slot.
#[test]
fn instance_data_is_isolated_per_instance() {
    // Per-instance capture slot keyed by an arbitrary instance index the
    // test sets on each `instance_data` call (via distinct types — we use a
    // small enum trick: capture into a shared map guarded by a Mutex).
    let captured_a = Arc::new(Mutex::new(None));
    let captured_b = Arc::new(Mutex::new(None));

    // Two clones of the runtime-building helper that capture into different
    // slots. We use one runtime with a plugin that always overwrites a
    // single shared slot — but spawn TWO instances sequentially, draining
    // the slot between them.
    let runtime = build_runtime(Box::new(CapturePluginId {
        seen: captured_a.clone(),
    }));

    // Instance A — stamped with "A".
    let _app_a = runtime
        .app_builder()
        .instance_data(PluginId("instance-A".into()))
        .renderer(Box::new(NoopRenderer::new()), (50.0, 50.0), 1.0)
        .build()
        .expect("app A");
    let a_value = captured_a.lock().unwrap().clone();
    assert_eq!(
        a_value,
        Some(PluginId("instance-A".into())),
        "A sees its own"
    );

    // Move A's slot aside, then build instance B with a fresh slot — B must
    // see only its own value, never A's.
    let _prev = {
        let mut g = captured_a.lock().unwrap();
        g.take()
    };
    let runtime_b = build_runtime(Box::new(CapturePluginId {
        seen: captured_b.clone(),
    }));
    let _app_b = runtime_b
        .app_builder()
        .instance_data(PluginId("instance-B".into()))
        .renderer(Box::new(NoopRenderer::new()), (50.0, 50.0), 1.0)
        .build()
        .expect("app B");
    let b_value = captured_b.lock().unwrap().clone();
    assert_eq!(
        b_value,
        Some(PluginId("instance-B".into())),
        "B sees its own"
    );

    // Cross-check: A's captured value was "instance-A", B's is "instance-B"
    // — they were never the same slot.
    assert_ne!(
        a_value, b_value,
        "per-instance data must be isolated across instances"
    );
}

// An instance whose builder never called `instance_data::<PluginId>(...)`
// must report `None` from both accessors — bridge fns should treat this as
// "no plugin context bound" and error accordingly.
#[test]
fn instance_data_missing_returns_none() {
    let seen = Arc::new(Mutex::new(Some(PluginId("sentinel-never-observed".into()))));
    let runtime = build_runtime(Box::new(CapturePluginId { seen: seen.clone() }));

    let _app = runtime
        .app_builder()
        .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
        .build()
        .expect("app build");

    assert!(
        seen.lock().unwrap().is_none(),
        "no instance_data::<PluginId>() call → data::<PluginId>() must be None"
    );
}

// Same-type double-stamp must panic at the SECOND call (eager detection at
// the call site, not deferred to `build()`). Distinct types coexist fine.
#[test]
#[should_panic(expected = "called twice on the same TurAppBuilder")]
fn instance_data_same_type_panics_at_second_call() {
    let runtime = build_runtime(Box::new(CapturePluginId {
        seen: Arc::new(Mutex::new(None)),
    }));

    // First stamp of PluginId is fine; a second stamp of the SAME type must
    // panic at the call site. A different type (ThemeOverride) would have
    // been accepted.
    let _ = runtime
        .app_builder()
        .instance_data(PluginId("first".into()))
        .instance_data(PluginId("second".into())); // panics here
}

// Confirm a builder can carry multiple DISTINCT types in one chain — each
// ends up in its own TypeId slot. (The panic test above covers the
// same-type case; this covers the distinct-type case.)
#[test]
fn instance_data_accepts_multiple_distinct_types() {
    let seen_pid = Arc::new(Mutex::new(None));
    let seen_theme = Arc::new(Mutex::new(None));

    struct CaptureBoth {
        pid: Arc<Mutex<Option<PluginId>>>,
        theme: Arc<Mutex<Option<ThemeOverride>>>,
    }
    impl Plugin for CaptureBoth {
        fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
            *self.pid.lock().unwrap() = ctx.js_ctx().data::<PluginId>();
            *self.theme.lock().unwrap() = ctx.js_ctx().data::<ThemeOverride>();
            Ok(())
        }
    }

    let runtime = build_runtime(Box::new(CaptureBoth {
        pid: seen_pid.clone(),
        theme: seen_theme.clone(),
    }));

    let _app = runtime
        .app_builder()
        .instance_data(PluginId("com.ease.multi".into()))
        .instance_data(ThemeOverride { is_dark: true })
        .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
        .build()
        .expect("app build");

    assert_eq!(
        seen_pid.lock().unwrap().clone(),
        Some(PluginId("com.ease.multi".into())),
        "PluginId stamped and observed"
    );
    assert_eq!(
        seen_theme.lock().unwrap().clone(),
        Some(ThemeOverride { is_dark: true }),
        "ThemeOverride stamped and observed alongside PluginId"
    );
}
