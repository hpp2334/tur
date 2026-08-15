//! Per-instance data slot: build-time `define` (via
//! `TurAppBuilder::instance_data`, closure runs on the worker before any
//! plugin `register`) seeds typed state; runtime `update` (replace) and
//! `data` / `with_data` (read) access it from plugin `register`, bridge
//! fns, or subsystem flush contexts. Never accessible to JS itself —
//! carries secure, JS-unforgeable identity (e.g. a host `PluginId` for
//! plugin systems where bridge fns must resolve the calling plugin without
//! trusting JS arguments).
//!
//! Define/update/read split (strict, fail-fast):
//! - `InstanceDataCx::define::<T>(v)` — build-time only; panics on duplicate.
//! - `TurInstanceContext::update::<T>(v)` — runtime; panics if `T` was not
//!   defined at build time.
//! - `TurInstanceContext::data::<T>()` / `with_data::<T, _>(f)` — runtime
//!   read; return `None` for unstamped types.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::js_runtime::InstanceDataCx;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;
use tur_engine::renderer::NoopRenderer;
use tur_integration_tests::MutexFixedClock;
use tur_integration_tests::TestSchedulerDriver;
use tur_native::NativeFontLoader;

// ---------- Test marker types ----------------------------------------------
//
// `PluginId` mirrors the canonical use case (a plugin stamps its own identity
// at build time so bridge fns can resolve per-plugin storage without
// trusting JS args). `ThemeOverride` is a second distinct type so we can
// exercise multi-typed stamping in one instance.

#[derive(Clone, Debug, PartialEq)]
struct PluginId(String);

#[derive(Clone, Debug, PartialEq)]
struct ThemeOverride {
    is_dark: bool,
}

// ---------- Helpers --------------------------------------------------------

/// Build a runtime carrying std + animation (no extra plugin).
fn build_runtime() -> Rc<TurRuntime> {
    TurRuntime::builder()
        .scheduler(TestSchedulerDriver::new())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .plugin(TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .build()
        .expect("runtime build")
}

/// Build a runtime carrying std + animation + the supplied extra plugin.
fn build_runtime_with(extra: Box<dyn Plugin>) -> Rc<TurRuntime> {
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

/// Build a headless app off the given runtime, with a build-time
/// `instance_data` definer closure. The closure runs on the worker before
/// any plugin `register`; plugins see all defined slots as already present.
fn build_headless_with_data<F>(runtime: &Rc<TurRuntime>, definer: F) -> Rc<tur_engine::TurApp>
where
    F: FnOnce(&mut InstanceDataCx) + Send + 'static,
{
    let app = runtime
        .app_builder()
        .instance_data(definer)
        .renderer(Box::new(NoopRenderer::new()))
        .view_root("main", (10.0, 10.0), 1.0)
        .build()
        .expect("app build");
    app.setup_root(
        "main",
        Box::new(tur_engine::renderer::noop::NoopSurface),
        (10.0, 10.0),
        1.0,
    )
    .expect("setup root");
    app
}

/// Build a headless app off the given runtime, with no build-time data
/// defined.
fn build_headless(runtime: &Rc<TurRuntime>) -> Rc<tur_engine::TurApp> {
    let app = runtime
        .app_builder()
        .renderer(Box::new(NoopRenderer::new()))
        .view_root("main", (10.0, 10.0), 1.0)
        .build()
        .expect("app build");
    app.setup_root(
        "main",
        Box::new(tur_engine::renderer::noop::NoopSurface),
        (10.0, 10.0),
        1.0,
    )
    .expect("setup root");
    app
}

// ---------- Plugins that exercise the worker-side API -----------------------

/// Reads `data::<PluginId>()` during `register` and captures the result.
struct ReadPluginId {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for ReadPluginId {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        *self.seen.lock().unwrap() = ctx.js_ctx().data::<PluginId>();
        Ok(())
    }
}

/// Reads `with_data::<PluginId, _>(f)` during `register` and captures the
/// result. Confirms the ref-callback path works without `T: Clone` on the
/// accessor side.
struct ReadPluginIdViaRef {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for ReadPluginIdViaRef {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let captured = ctx.js_ctx().with_data::<PluginId, _>(|pid| pid.clone());
        *self.seen.lock().unwrap() = captured;
        Ok(())
    }
}

/// Reads two distinct types during `register` — confirms the map carries
/// distinct TypeId slots.
struct ReadMultipleTypes {
    pid: Arc<Mutex<Option<PluginId>>>,
    theme: Arc<Mutex<Option<ThemeOverride>>>,
}
impl Plugin for ReadMultipleTypes {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let js = ctx.js_ctx();
        *self.pid.lock().unwrap() = js.data::<PluginId>();
        *self.theme.lock().unwrap() = js.data::<ThemeOverride>();
        Ok(())
    }
}

/// Calls `update::<PluginId>(v)` during `register`, then reads it back.
struct UpdatePluginId {
    seen: Arc<Mutex<Option<PluginId>>>,
}
impl Plugin for UpdatePluginId {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let js = ctx.js_ctx();
        js.update::<PluginId>(PluginId("updated".into()));
        *self.seen.lock().unwrap() = js.data::<PluginId>();
        Ok(())
    }
}

/// Calls `update::<PluginId>(v)` during `register` without a prior
/// build-time `define` — must panic.
struct UpdateUndefinedPluginId;
impl Plugin for UpdateUndefinedPluginId {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        ctx.js_ctx()
            .update::<PluginId>(PluginId("never-defined".into()));
        Ok(())
    }
}

// ---------- Tests: build-time define + runtime read ------------------------

// Build-time `define` seeds `PluginId`; a plugin reads it back via the
// clone-out accessor (`data::<T>()`) — the canonical round-trip.
#[test]
fn define_then_read_via_data_clone_out() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime_with(Box::new(ReadPluginId { seen: seen.clone() }));
    let _app = build_headless_with_data(&runtime, |cx| {
        cx.define::<PluginId>(PluginId("com.ease.onedrive".into()));
    });

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("com.ease.onedrive".into())),
        "plugin should observe the build-time define via data::<T>()",
    );
}

// Same round-trip via the ref-callback accessor (`with_data`). Confirms both
// read shapes work and that `with_data` does not require `T: Clone` on the
// accessor side.
#[test]
fn define_then_read_via_with_data_ref_callback() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime_with(Box::new(ReadPluginIdViaRef { seen: seen.clone() }));
    let _app = build_headless_with_data(&runtime, |cx| {
        cx.define::<PluginId>(PluginId("com.ease.spotify".into()));
    });

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("com.ease.spotify".into())),
        "with_data ref-callback should observe the build-time define",
    );
}

// Two instances from two runtimes, each with its own build-time `define` of a
// distinct `PluginId`. Each instance's plugin must see ONLY its own value —
// per-instance isolation, not a shared runtime-wide slot.
#[test]
fn define_is_isolated_per_instance() {
    let captured_a = Arc::new(Mutex::new(None));
    let captured_b = Arc::new(Mutex::new(None));

    let runtime_a = build_runtime_with(Box::new(ReadPluginId {
        seen: captured_a.clone(),
    }));
    let _app_a = build_headless_with_data(&runtime_a, |cx| {
        cx.define::<PluginId>(PluginId("instance-A".into()));
    });
    let a_value = captured_a.lock().unwrap().clone();
    assert_eq!(
        a_value,
        Some(PluginId("instance-A".into())),
        "A sees its own"
    );

    let runtime_b = build_runtime_with(Box::new(ReadPluginId {
        seen: captured_b.clone(),
    }));
    let _app_b = build_headless_with_data(&runtime_b, |cx| {
        cx.define::<PluginId>(PluginId("instance-B".into()));
    });
    let b_value = captured_b.lock().unwrap().clone();
    assert_eq!(
        b_value,
        Some(PluginId("instance-B".into())),
        "B sees its own"
    );

    assert_ne!(
        a_value, b_value,
        "per-instance data must be isolated across instances",
    );
}

// A plugin that reads `data::<PluginId>()` with no build-time `define` must
// observe `None` — bridge fns should treat this as "no plugin context bound"
// and error accordingly.
#[test]
fn data_returns_none_when_nothing_defined() {
    let seen = Arc::new(Mutex::new(Some(PluginId("sentinel-never-observed".into()))));
    let runtime = build_runtime_with(Box::new(ReadPluginId { seen: seen.clone() }));
    let _app = build_headless(&runtime);

    assert!(
        seen.lock().unwrap().is_none(),
        "no define::<PluginId>() at build time → data::<PluginId>() must be None",
    );
}

// Confirm build-time definer can stamp multiple DISTINCT types in one
// closure — each ends up in its own TypeId slot.
#[test]
fn define_accepts_multiple_distinct_types() {
    let seen_pid = Arc::new(Mutex::new(None));
    let seen_theme = Arc::new(Mutex::new(None));

    let runtime = build_runtime_with(Box::new(ReadMultipleTypes {
        pid: seen_pid.clone(),
        theme: seen_theme.clone(),
    }));
    let _app = build_headless_with_data(&runtime, |cx| {
        cx.define::<PluginId>(PluginId("com.ease.multi".into()));
        cx.define::<ThemeOverride>(ThemeOverride { is_dark: true });
    });

    assert_eq!(
        seen_pid.lock().unwrap().clone(),
        Some(PluginId("com.ease.multi".into())),
        "PluginId defined and observed",
    );
    assert_eq!(
        seen_theme.lock().unwrap().clone(),
        Some(ThemeOverride { is_dark: true }),
        "ThemeOverride defined and observed alongside PluginId",
    );
}

// ---------- Tests: runtime update ------------------------------------------

// `update::<T>(v)` at runtime replaces the build-time-defined value; a
// subsequent `data::<T>()` observes the replacement.
#[test]
fn update_replaces_existing_value() {
    let seen = Arc::new(Mutex::new(None));
    let runtime = build_runtime_with(Box::new(UpdatePluginId { seen: seen.clone() }));
    let _app = build_headless_with_data(&runtime, |cx| {
        cx.define::<PluginId>(PluginId("original".into()));
    });

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some(PluginId("updated".into())),
        "update should replace the build-time value",
    );
}

// `update::<T>(v)` for a `TypeId` not defined at build time must panic
// (fail-fast — catches a missing build-time `define` immediately).
#[test]
#[should_panic]
fn update_undefined_type_panics() {
    let runtime = build_runtime_with(Box::new(UpdateUndefinedPluginId));
    // No .instance_data(...) → PluginId was never defined → update panics.
    let _app = build_headless(&runtime);
}

// ---------- Tests: strict define (duplicate panics) ------------------------

// `define::<T>(v)` called twice for the same `TypeId` inside the builder
// closure must panic (each type may be defined exactly once per instance).
#[test]
#[should_panic]
fn define_same_type_panics() {
    let runtime = build_runtime();
    let _app = build_headless_with_data(&runtime, |cx| {
        cx.define::<PluginId>(PluginId("first".into()));
        cx.define::<PluginId>(PluginId("second".into())); // panics
    });
}
