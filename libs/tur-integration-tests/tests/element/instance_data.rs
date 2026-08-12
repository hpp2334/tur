//! Worker-side per-instance data slot: `TurInstanceContext::insert_data::<T>(value)`
//! stamps typed state from a plugin's `register` (or any later point on the
//! worker); `TurInstanceContext::data::<T>()` (clone-out) and
//! `TurInstanceContext::with_data::<T, _>(f)` (ref-callback) read it from bridge fns
//! or subsystem flush contexts. Never accessible to JS itself — carries
//! secure, JS-unforgeable identity (e.g. a `PluginId` for the Ease Music
//! Player plugin system, so a `storage.get(key)` bridge can resolve the
//! calling plugin without trusting JS args).
//!
//! The data map lives entirely in the worker; there is no embedder-facing
//! API to populate it from the main thread.

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
// `PluginId` mirrors the canonical use case (a plugin stamps its own identity
// at register-time so bridge fns can resolve per-plugin storage without
// trusting JS args). `ThemeOverride` is a second distinct type so we can
// exercise multi-typed stamping in one instance.

#[derive(Clone, Debug, PartialEq)]
struct PluginId(String);

#[derive(Clone, Debug, PartialEq)]
struct ThemeOverride {
    is_dark: bool,
}

// ---------- Helpers --------------------------------------------------------

/// Build a runtime carrying std + animation + the supplied extra plugin.
/// Each test supplies a plugin that interacts with `TurInstanceContext` at
/// register-time.
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

/// Build a headless app off the given runtime. The instance's plugins have
/// already run their `register` by the time `build()` returns.
fn build_headless(runtime: &Rc<TurRuntime>) -> Rc<tur_engine::TurApp> {
    runtime
        .app_builder()
        .renderer(Box::new(NoopRenderer::new()), (10.0, 10.0), 1.0)
        .build()
        .expect("app build")
}

// ---------- Plugins that exercise the worker-side API -----------------------

/// Stamps `PluginId` via `insert_data` during `register`, then reads it back
/// via the clone-out accessor (`data::<T>()`). Captures the read-back value
/// for the test to assert on.
struct StampAndReadPluginId {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for StampAndReadPluginId {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let js = ctx.js_ctx();
        js.insert_data(PluginId("com.ease.onedrive".into()));
        *self.seen.lock().unwrap() = js.data::<PluginId>();
        Ok(())
    }
}

/// Stamps `PluginId` via `insert_data`, then reads it back via the
/// ref-callback accessor (`with_data::<T, _>(f)`). Confirms the callback
/// path works and does not require `T: Clone` on the `TurInstanceContext` side.
struct StampAndReadViaRef {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for StampAndReadViaRef {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let js = ctx.js_ctx();
        js.insert_data(PluginId("com.ease.spotify".into()));
        let captured = js.with_data::<PluginId, _>(|pid| pid.clone());
        *self.seen.lock().unwrap() = captured;
        Ok(())
    }
}

/// Reads `data::<PluginId>()` without any prior `insert_data` call — must
/// observe `None`. Bridge fns treat this as "no plugin context bound" and
/// error accordingly.
struct ReadOnlyPlugin {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for ReadOnlyPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        *self.seen.lock().unwrap() = ctx.js_ctx().data::<PluginId>();
        Ok(())
    }
}

/// Stamps two distinct types in one `register` (PluginId + ThemeOverride),
/// then reads both back — confirms the map carries distinct TypeId slots.
struct StampMultipleTypes {
    pid: Arc<Mutex<Option<PluginId>>>,
    theme: Arc<Mutex<Option<ThemeOverride>>>,
}
impl Plugin for StampMultipleTypes {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let js = ctx.js_ctx();
        js.insert_data(PluginId("com.ease.multi".into()));
        js.insert_data(ThemeOverride { is_dark: true });
        *self.pid.lock().unwrap() = js.data::<PluginId>();
        *self.theme.lock().unwrap() = js.data::<ThemeOverride>();
        Ok(())
    }
}

// ---------- Tests ----------------------------------------------------------

// Round-trip: a plugin stamps `PluginId` via `insert_data`, then reads it
// back via the clone-out accessor — all inside `register`.
#[test]
fn insert_data_round_trip_via_data_clone_out() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime(Box::new(StampAndReadPluginId { seen: seen.clone() }));
    let _app = build_headless(&runtime);

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("com.ease.onedrive".into())),
        "plugin should observe its own insert_data stamp in the same register call"
    );
}

// Same round-trip via the ref-callback accessor (`with_data`). Confirms both
// read shapes work and that `with_data` does not require `T: Clone` on the
// `TurInstanceContext` side.
#[test]
fn insert_data_round_trip_via_with_data_ref_callback() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime(Box::new(StampAndReadViaRef { seen: seen.clone() }));
    let _app = build_headless(&runtime);

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("com.ease.spotify".into())),
        "with_data ref-callback should observe the stamped value"
    );
}

// Two instances from one runtime, each with its own plugin that stamps a
// distinct `PluginId`. Each instance's `register` must see ONLY its own
// value — per-instance isolation, not a shared runtime-wide slot.
#[test]
fn insert_data_is_isolated_per_instance() {
    let captured_a = Arc::new(Mutex::new(None));
    let captured_b = Arc::new(Mutex::new(None));

    // Plugin A stamps "instance-A".
    struct StampFixed {
        value: PluginId,
        seen: Arc<Mutex<Option<PluginId>>>,
    }
    impl Plugin for StampFixed {
        fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
            let js = ctx.js_ctx();
            js.insert_data(self.value.clone());
            *self.seen.lock().unwrap() = js.data::<PluginId>();
            Ok(())
        }
    }

    let runtime = build_runtime(Box::new(StampFixed {
        value: PluginId("instance-A".into()),
        seen: captured_a.clone(),
    }));
    let _app_a = build_headless(&runtime);
    let a_value = captured_a.lock().unwrap().clone();
    assert_eq!(
        a_value,
        Some(PluginId("instance-A".into())),
        "A sees its own"
    );

    // Fresh runtime + plugin B stamps "instance-B". B must see only its own
    // value, never A's.
    let runtime_b = build_runtime(Box::new(StampFixed {
        value: PluginId("instance-B".into()),
        seen: captured_b.clone(),
    }));
    let _app_b = build_headless(&runtime_b);
    let b_value = captured_b.lock().unwrap().clone();
    assert_eq!(
        b_value,
        Some(PluginId("instance-B".into())),
        "B sees its own"
    );

    assert_ne!(
        a_value, b_value,
        "per-instance data must be isolated across instances"
    );
}

// A plugin that reads `data::<PluginId>()` without any prior `insert_data`
// must observe `None` — bridge fns should treat this as "no plugin context
// bound" and error accordingly.
#[test]
fn data_returns_none_when_nothing_stamped() {
    let seen = Arc::new(Mutex::new(Some(PluginId("sentinel-never-observed".into()))));
    let runtime = build_runtime(Box::new(ReadOnlyPlugin { seen: seen.clone() }));
    let _app = build_headless(&runtime);

    assert!(
        seen.lock().unwrap().is_none(),
        "no insert_data::<PluginId>() call → data::<PluginId>() must be None"
    );
}

// Confirm a plugin can stamp multiple DISTINCT types in one register — each
// ends up in its own TypeId slot.
#[test]
fn insert_data_accepts_multiple_distinct_types() {
    let seen_pid = Arc::new(Mutex::new(None));
    let seen_theme = Arc::new(Mutex::new(None));

    let runtime = build_runtime(Box::new(StampMultipleTypes {
        pid: seen_pid.clone(),
        theme: seen_theme.clone(),
    }));
    let _app = build_headless(&runtime);

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

// `insert_data` of the same TypeId twice silently overwrites (mirrors
// `Capabilities::insert`). This pins the documented behaviour: no panic, last
// write wins.
#[test]
fn insert_data_same_type_silently_overwrites() {
    struct StampTwice {
        seen: Arc<Mutex<Option<PluginId>>>,
    }
    impl Plugin for StampTwice {
        fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
            let js = ctx.js_ctx();
            js.insert_data(PluginId("first".into()));
            js.insert_data(PluginId("second".into())); // overwrites "first"
            *self.seen.lock().unwrap() = js.data::<PluginId>();
            Ok(())
        }
    }

    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime(Box::new(StampTwice { seen: seen.clone() }));
    let _app = build_headless(&runtime);

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("second".into())),
        "second insert_data of the same TypeId should overwrite the first"
    );
}
