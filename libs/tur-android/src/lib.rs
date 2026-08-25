//! tur Android JNI runtime — the native side of the Compose integration.
//!
//! Exposes a C ABI (under the `Java_org_tur_TurNative_*` JNI names) that Kotlin
//! (`integrations/compose`) calls to build, drive, and tear down a tur engine
//! instance. The engine itself, the renderer, and all plugins come from
//! `tur-engine` + `tur-animation` + plugins unchanged; this crate is
//! only the embedder glue (surface, events, loop driver) — the same three
//! integration seams the wasm and native harnesses use.
//!
//! ## Threading: the tur-host thread
//!
//! Every piece of `!Send` embedder state (the runtime, the wgpu
//! renderer, the frame-loop future, the host-side drain) lives on one
//! dedicated **tur-host thread** (see [`host_thread`]); the JNI entry
//! points are thin marshalling stubs that post closures onto its FIFO op
//! queue. The Android main thread never runs engine work — per-frame
//! `pump`s (GPU encode + present), instance builds (wgpu adapter/device
//! init + the worker-lane handshake), module loads, and teardown all run
//! on the tur-host thread, ordered behind one another. The only
//! main-thread touchpoints left are the Choreographer callback (a trivial
//! post) and the JNI trampolines themselves.
//!
//! On non-Android targets the crate compiles as a stub (the JNI entry points
//! are gated), so the workspace still builds on desktop for `cargo check`.

// On non-Android targets the whole crate is an unreachable stub: the JNI
// entry points (the only callers of `init_logger_once`, `AndroidApp::create`,
// the surface helpers, etc.) are `cfg(target_os = "android")`-gated, so the
// bodies would otherwise be flagged as dead code. Allow it.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

mod app;
mod host_thread;
pub mod scheduler;
mod surface;

// Re-exported for embedder convenience — the registry itself is engine-owned
// (`tur_engine::ModuleSourceRegistry`); the Android glue only stores and
// drives it (registerModuleSource / releaseModuleSource JNI ops).
pub use tur_engine::ModuleSourceRegistry;

// Re-export the JNI primitive types so the `standard_jni_exports!()` macro
// (expanded inside an embedder's cdylib) can name them via `$crate::…` without
// forcing the embedder to add the `jni` crate as a direct dependency.
pub use jni::JNIEnv;
pub use jni::objects::{JClass, JObject, JString};
pub use jni::sys::{jboolean, jdouble, jint, jlong};

use std::sync::OnceLock;

/// The process `JavaVM`, stashed on the first JNI call so the clipboard backend
/// and the loop driver can attach the frame thread as needed. Held behind a
/// `Box` because `JavaVM` is not `Clone` (it wraps a raw `*mut sys::JavaVM`),
/// so we hand out `&'static` borrows instead.
static JAVA_VM: OnceLock<Box<jni::JavaVM>> = OnceLock::new();

/// Access the process `JavaVM` (set on the first JNI entry) as a `'static`
/// borrow. Returns `None` before the first JNI call has run.
pub(crate) fn java_vm() -> Option<&'static jni::JavaVM> {
    JAVA_VM.get().map(|b| &**b)
}

/// Read an Android system property (`adb shell setprop <name> <value>`). Used to
/// gate on-device crash diagnostics without rebuilding — e.g. `debug.tur.crash`
/// triggers a deliberate panic inside `pump` to verify the panic hook surfaces a
/// readable backtrace in logcat. Returns `None` on Android-not-present or unset.
#[cfg(target_os = "android")]
fn system_prop(name: &str) -> Option<String> {
    use std::ffi::CString;
    use std::os::raw::c_char;
    unsafe extern "C" {
        fn __system_property_get(name: *const c_char, value: *mut c_char) -> i32;
    }
    let c_name = CString::new(name).ok()?;
    // PROP_VALUE_MAX == 92.
    let mut buf = [0 as c_char; 92];
    let len = unsafe { __system_property_get(c_name.as_ptr(), buf.as_mut_ptr()) };
    if len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len as usize) };
        Some(String::from_utf8_lossy(bytes).into_owned())
    } else {
        None
    }
}

#[cfg(not(target_os = "android"))]
fn system_prop(_name: &str) -> Option<String> {
    None
}

/// Initialize logcat logging once on the first JNI call, so the engine's
/// `tracing::info!`/`error!` output lands in logcat (tagged `tur`).
///
/// On Android this installs `android_logger` as the `log::Log` impl and a
/// `tracing_subscriber` whose `MakeWriter` routes every `tracing` event line
/// through that `log::Log` impl — so engine `tracing::*` macros reach logcat
/// instead of stderr (which is invisible on Android). On other targets the
/// `tracing_subscriber` writes to stderr as usual for `cargo check`.
#[cfg(target_os = "android")]
mod logger {
    /// A `tracing_subscriber` `MakeWriter` that routes each event line to
    /// `android_logger` (the `log::Log` impl → logcat under the `tur` tag).
    struct LogcatWriter(log::Level);

    impl std::io::Write for LogcatWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let s = String::from_utf8_lossy(buf);
            // `tracing_subscriber::fmt` emits one event per write call (with a
            // trailing newline), so log the whole buffer as one record.
            log::logger().log(
                &log::Record::builder()
                    .level(self.0)
                    .target("tur")
                    .args(format_args!("{}", s.trim_end()))
                    .build(),
            );
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct LogcatMakeWriter;

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogcatMakeWriter {
        type Writer = LogcatWriter;
        fn make_writer(&'a self) -> Self::Writer {
            // fmt events always carry a level; the API requires a fallback.
            LogcatWriter(log::Level::Info)
        }
    }

    pub(super) fn init() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            // android_logger → logcat (the `log` crate's global logger, tag "tur").
            // Floor at `info`, with the chatty GPU/JNI deps (`jni`, `wgpu_core`,
            // `naga`, `wgpu_hal`) pushed to `warn` — they emit via `log::*`
            // directly (not tracing), so the tracing EnvFilter below doesn't
            // gate them; a single frame otherwise floods logcat with thousands
            // of `jni` trace lines.
            android_logger::init_once(
                android_logger::Config::default()
                    .with_tag("tur")
                    .with_max_level(log::LevelFilter::Trace)
                    .with_filter(
                        android_logger::FilterBuilder::new()
                            .parse("info,jni=warn,wgpu_core=warn,wgpu_hal=warn,naga=warn")
                            .build(),
                    ),
            );

            // tracing → log bridge: route every `tracing` event through the
            // `log::Log` impl above so engine `tracing::*` macros reach logcat.
            //
            // The default floor is `warn` so the chatty dependency crates
            // (`jni`, `wgpu_core`, `naga`, `wgpu_hal`) don't flood logcat at
            // info/trace — a single frame emits thousands of `jni` trace lines
            // otherwise. Our own crates (`tur`, `tur_engine`, `tur_android`,
            // `tur_animation`) opt back up to `info` so boot + frame milestones
            // are visible.
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new(
                    "warn,tur=info,tur_engine=info,tur_android=info,tur_animation=info",
                ))
                .with_writer(LogcatMakeWriter)
                .finish();
            tracing::subscriber::set_global_default(subscriber).ok();

            // Panic hook → logcat. On Android, stderr is `/dev/null`, so the
            // default panic hook (which writes to stderr) makes panic messages
            // invisible — a panic becomes an opaque SIGABRT. Install a hook that
            // logs the panic payload + location + a full `std::backtrace` at
            // ERROR, then defers to the previous hook for the abort/backtrace.
            // The backtrace is what surfaces the actual panicking call path
            // (function names resolve from the release `.symtab`; the abort
            // tombstone alone only shows the panic machinery). Each backtrace
            // line is logged separately because android_logger truncates very
            // long single messages.
            let prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let location = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_else(|| "<unknown>".into());
                let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = info.payload().downcast_ref::<String>() {
                    s.clone()
                } else {
                    "<non-string panic payload>".to_string()
                };
                log::error!("PANIC at {location}: {payload}");
                let bt = std::backtrace::Backtrace::force_capture();
                log::error!("PANIC backtrace:");
                for line in format!("{bt}").lines() {
                    log::error!("  {line}");
                }
                prev_hook(info);
            }));

            tracing::info!("tur android logger initialized");
        });
    }
}

#[cfg(not(target_os = "android"))]
mod logger {
    pub(super) fn init() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            // On desktop (cargo check / unit tests), tracing → stderr.
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .with_writer(std::io::stderr)
                .finish();
            tracing::subscriber::set_global_default(subscriber).ok();
        });
    }
}

fn init_logger_once() {
    logger::init();
}

/// Standard engine operations — the handle-based API the Kotlin integration
/// drives (create instances, load JS, pump a frame, push input, query/edit
/// IME, tear down).
///
/// The model is **one runtime, many instances**:
/// - [`create_runtime`] builds the shared [`AndroidRuntime`] (fonts, clock,
///   capabilities, plugins, wgpu instance) — no surface. Returns a runtime
///   handle.
/// - [`create_instance`] / [`create_headless_instance`] spawn an isolated
///   [`AndroidInstance`] from a runtime handle (rendering / headless). Returns
///   an instance handle.
/// - [`register_module_source`] / [`release_module_source`] /
///   `load_module`/`pump`/`resize`/`push_*`/`destroy`
///   operate on **instance** handles (module sources register on the
///   **runtime**, then load into any of its instances by handle). (Text-
///   input state is pushed to Kotlin via `FrameLoop.onTextInputChanged`
///   from the engine's shell — no `focused_is_editable` JNI poll.)
/// - [`destroy_runtime`] drops the runtime.
///
/// ## Threading
///
/// Every op marshals onto the **tur-host thread** ([`host_thread`]) — the
/// single thread that owns the runtime, the instances, the renderers, and
/// the frame-loop futures (all `!Send`). The op channel is FIFO, which
/// gives total ordering: an op can never observe a half-built instance,
/// and `destroy` always lands after the create it follows. Fire-and-forget
/// ops (`pump`, input, resize, `load_module`, instance creation, destroy)
/// return immediately on the calling (usually the Android main) thread;
/// ops that must return a value (`register_module_source` for the registry
/// — actually caller-thread via the Arc-shared registry — and the
/// `with_app` / `with_runtime` escape hatches) block only for their own
/// round-trip. Native build failures inside a posted op surface as logcat
/// errors, and every later op for that handle becomes an ordered no-op.
///
/// Runtime creation varies per embedder (plugin set), so [`create_runtime`]
/// takes a `configure` closure (which must be `Send` — it crosses onto the
/// host thread; all `Plugin`s are `Send + Sync` by trait) and is called
/// from the embedder's own `Java_<pkg>_<Class>_createRuntime` JNI function.
/// The instance/runtime ops are standard and generated by
/// [`standard_jni_exports!`].
#[cfg(target_os = "android")]
pub mod ops {
    use std::ffi::c_void;

    use jni::JNIEnv;
    use jni::objects::{JObject, JString};
    use jni::sys::{jdouble, jint, jlong};
    use tur_engine::core::layout::{MouseButton, Offset};
    use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
    use tur_engine::core::platform::{ImeEvent, PointerDeviceKind, PointerInput};
    use tur_engine::core::shell::ShellEvent;
    use tur_engine::{TurApp, TurAppBuilder, TurRuntimeBuilder};

    use crate::app::{AndroidInstance, AndroidRuntime};
    use crate::host_thread::{InstanceRoute, RuntimeRoute};

    /// `createRuntime(env, context): long`
    ///
    /// Builds the shared runtime (no surface) and returns an opaque pointer
    /// handle (a boxed [`RuntimeRoute`]) Kotlin holds as a `long`. Called
    /// from the embedder's own `Java_<pkg>_<Class>_createRuntime` JNI
    /// function, which passes a `configure` closure that adds the embedder's
    /// plugin set.
    ///
    /// Spawns the runtime's **tur-host thread** and builds the runtime on
    /// it, waiting for completion so a broken build fails fast here (the
    /// historical throw-on-failure contract). This is a one-time,
    /// comparatively cheap step (font context + wgpu instance + plugin
    /// `compile`) — the heavy per-instance work (wgpu adapter/device) is
    /// the async part, in [`create_instance`].
    ///
    /// Returns `0` on failure (a `RuntimeException` is also thrown).
    pub fn create_runtime(
        env: &mut JNIEnv,
        context: JObject,
        configure: impl FnOnce(TurRuntimeBuilder) -> TurRuntimeBuilder + Send + 'static,
    ) -> jlong {
        catch_into_zero(env, "createRuntime", |env| {
            crate::init_logger_once();
            stash_java_vm(env)?;
            let context_ref = env.new_global_ref(context)?;
            log::info!("createRuntime: spawning tur-host thread + building shared runtime");
            let (host, join) = crate::host_thread::spawn();
            // The shared module-source registry is allocated on THIS thread
            // and cloned into the runtime once built — both halves share
            // entries (Arc), so registerModuleSource stays a cheap
            // caller-thread op that never waits for the host thread.
            let module_sources = crate::ModuleSourceRegistry::new();
            let ms_for_build = module_sources.clone();
            let built = host.call(move |state| {
                match AndroidRuntime::build(context_ref, ms_for_build, configure) {
                    Ok(runtime) => {
                        state.set_runtime(Box::new(runtime));
                        Ok(())
                    }
                    Err(e) => Err(e.to_string()),
                }
            });
            match built {
                Ok(Ok(())) => {}
                Ok(Err(msg)) => return Err(format!("createRuntime: {msg}").into()),
                Err(msg) => return Err(format!("createRuntime: {msg}").into()),
            }
            log::info!("createRuntime: runtime built OK (tur-host thread)");
            let route = RuntimeRoute {
                host,
                module_sources,
                join: Some(join),
            };
            Ok(Box::into_raw(Box::new(route)) as jlong)
        })
    }

    /// `createInstance(env, runtimeHandle, surface, width, height, dpr, frameLoop): long`
    ///
    /// Spawns an isolated rendering instance attached to the given Android
    /// `Surface`, sharing the runtime's fonts/clock/capabilities/wgpu-instance.
    /// Returns an instance handle **immediately** — the heavy build (wgpu
    /// surface + adapter/device init + the worker-lane handshake + plugin
    /// registration) is posted to the **tur-host thread**, keeping the
    /// Android main thread free. FIFO op order guarantees every later op
    /// for this handle lands after the build; a failed build logs to logcat
    /// and turns those ops into no-ops (a blank surface, not a crash).
    ///
    /// The `ANativeWindow*` is acquired on the calling thread (inside
    /// `surfaceCreated`, where the Surface is guaranteed valid); the
    /// refcounted native window then stays memory-safe even if
    /// `surfaceDestroyed` races ahead of the host-thread build.
    ///
    /// `configure_instance` receives the [`TurAppBuilder`] before the
    /// surface-backed renderer is attached — chain
    /// [`TurAppBuilder::instance_data`] on it and return it to stamp
    /// per-instance data at build time. It must be `Send + 'static` (it
    /// crosses onto the host thread). The standard
    /// [`standard_jni_exports!`](crate::standard_jni_exports!) trampoline
    /// passes `|b| b` (no-op); embedders that need build-time data write
    /// their own `Java_<pkg>_<Class>_createInstance` (mirroring
    /// `createRuntime` — see the compose demo).
    #[allow(clippy::too_many_arguments)]
    pub fn create_instance(
        env: &mut JNIEnv,
        runtime_handle: jlong,
        surface: JObject,
        width: jint,
        height: jint,
        dpr: jdouble,
        frame_loop: JObject,
        configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a> + Send + 'static,
    ) -> jlong {
        catch_into_zero(env, "createInstance", |env| {
            let route = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            let surface_ref = env.new_global_ref(surface)?;
            let frame_loop_ref = env.new_global_ref(frame_loop)?;
            // Acquire the ANativeWindow* from the Surface NOW (this thread).
            let env_ptr = env.get_raw();
            let surface_ptr = surface_ref.as_raw();
            let anw = unsafe {
                crate::surface::native_window_from_surface(
                    env_ptr as *mut c_void,
                    surface_ptr as *mut c_void,
                )
            };
            drop(surface_ref);
            if anw.is_null() {
                return Err("ANativeWindow_fromSurface returned null".into());
            }
            let window_handle = unsafe { crate::surface::AndroidWindowHandle::new(anw) };
            let frame_loop_handle = crate::scheduler::FrameLoopRef::new(frame_loop_ref);
            let id = crate::host_thread::next_instance_id();
            let host = route.host.clone();
            let host_for_build = route.host.clone();
            log::info!(
                "createInstance: queued instance {id} build ({}x{} @{}x) on tur-host thread",
                width,
                height,
                dpr
            );
            let posted = host.post(move |state| {
                let Some(runtime) = state.runtime() else {
                    log::error!(
                        "createInstance: runtime gone during create — instance {id} not built"
                    );
                    return;
                };
                let result = pollster::block_on(AndroidInstance::build_with_surface(
                    runtime,
                    runtime.default_worker_pool.clone(),
                    &runtime.tokio_handle(),
                    &runtime.wgpu_instance,
                    window_handle,
                    width.max(1) as u32,
                    height.max(1) as u32,
                    dpr.max(1.0),
                    frame_loop_handle,
                    host_for_build.clone(),
                    id,
                    configure_instance,
                ));
                match result {
                    Ok(instance) => {
                        state.insert_instance(id, Box::new(instance));
                        log::info!("createInstance: instance {id} built OK");
                    }
                    Err(e) => {
                        log::error!("createInstance: instance {id} build failed: {e}");
                    }
                }
            });
            if !posted {
                return Err("tur-host thread has shut down".into());
            }
            let route = InstanceRoute { host, id };
            Ok(Box::into_raw(Box::new(route)) as jlong)
        })
    }

    /// `createHeadlessInstance(env, runtimeHandle, frameLoop): long`
    ///
    /// Spawns an isolated headless instance (no surface, no rendering) from
    /// the runtime. Like [`create_instance`], the build is posted to the
    /// tur-host thread and the handle is valid immediately; build failures
    /// log and turn later ops into no-ops.
    ///
    /// `configure_instance` mirrors [`create_instance`]'s hook — chain
    /// [`TurAppBuilder::instance_data`] on the builder and return it. It
    /// must be `Send + 'static`.
    pub fn create_headless_instance(
        env: &mut JNIEnv,
        runtime_handle: jlong,
        frame_loop: JObject,
        configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a> + Send + 'static,
    ) -> jlong {
        catch_into_zero(env, "createHeadlessInstance", |env| {
            let route = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            let frame_loop_ref = env.new_global_ref(frame_loop)?;
            let frame_loop_handle = crate::scheduler::FrameLoopRef::new(frame_loop_ref);
            let id = crate::host_thread::next_instance_id();
            let host = route.host.clone();
            let host_for_build = route.host.clone();
            log::info!("createHeadlessInstance: queued instance {id} build on tur-host thread");
            let posted = host.post(move |state| {
                let Some(runtime) = state.runtime() else {
                    log::error!(
                        "createHeadlessInstance: runtime gone during create — instance {id} not built"
                    );
                    return;
                };
                match AndroidInstance::build_headless(
                    runtime,
                    runtime.default_worker_pool.clone(),
                    &runtime.tokio_handle(),
                    frame_loop_handle,
                    host_for_build.clone(),
                    id,
                    configure_instance,
                ) {
                    Ok(instance) => {
                        state.insert_instance(id, Box::new(instance));
                        log::info!("createHeadlessInstance: instance {id} built OK");
                    }
                    Err(e) => {
                        log::error!("createHeadlessInstance: instance {id} build failed: {e}");
                    }
                }
            });
            if !posted {
                return Err("tur-host thread has shut down".into());
            }
            let route = InstanceRoute { host, id };
            Ok(Box::into_raw(Box::new(route)) as jlong)
        })
    }

    /// `registerModuleSource(env, runtimeHandle, js): long`
    ///
    /// Register a module source on the runtime's shared
    /// [`ModuleSourceRegistry`] and return its opaque handle (`0` on
    /// failure). The source crosses JNI exactly once, here — `loadModule`
    /// then loads it into any instance of the runtime by handle. Rust
    /// embedders skip even this hop: read the source natively and register
    /// it via [`with_runtime`].
    ///
    /// The registry is `Arc`-shared between this route and the host-thread
    /// runtime, so this runs directly on the calling thread — no host
    /// round-trip, and it works even while an instance build is queued.
    pub fn register_module_source(env: &mut JNIEnv, runtime_handle: jlong, js: JString) -> jlong {
        catch_into_zero(env, "registerModuleSource", |env| {
            let route = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            let js: String = env.get_string(&js)?.into();
            Ok(route.module_sources.register(js) as jlong)
        })
    }

    /// `releaseModuleSource(env, runtimeHandle, sourceHandle)`
    ///
    /// Drop a registered module source. Idempotent — an unknown/stale handle
    /// is a no-op (handles are monotonic ids, never reused). Everything left
    /// registered is released wholesale when the runtime is destroyed.
    pub fn release_module_source(env: &mut JNIEnv, runtime_handle: jlong, source_handle: jlong) {
        catch_void(env, "releaseModuleSource", |_env| {
            let route = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            route.module_sources.remove(source_handle as u64);
            Ok(())
        })
    }

    /// Evaluate the registered module source `source_handle` as an ES module
    /// (resolved by the engine's `TurModuleLoader` — `tur:std`,
    /// `tur:animation`, etc. must already be registered, which instance
    /// creation does), then request a paint so the bundle renders on the
    /// next frame.
    ///
    /// Posted to the tur-host thread (FIFO behind the instance build), so
    /// the calling thread returns immediately; a failed load logs to logcat
    /// instead of throwing. The registry's `Arc<str>` flows to the worker
    /// by refcount — no copy, no JNI string traffic. A source produced on
    /// the Rust side (e.g. an APK asset read via `AAssetManager`)
    /// therefore reaches the JS realm without ever being serialized across
    /// the JNI boundary.
    pub fn load_module(env: &mut JNIEnv, handle: jlong, source_handle: jlong) {
        catch_void(env, "loadModule", |_env| {
            let route = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let id = route.id;
            let posted = route.host.post(move |state| {
                let Some(instance) = state.instance(id) else {
                    log::warn!(
                        "loadModule: instance {id} not present (build failed or destroyed) — load skipped"
                    );
                    return;
                };
                log::info!(
                    "loadModule: source handle {source_handle} ({} bytes)",
                    instance
                        .module_sources
                        .get(source_handle as u64)
                        .map(|s| s.len())
                        .unwrap_or(0)
                );
                match futures::executor::block_on(
                    instance
                        .app
                        .load_module_source(&instance.module_sources, source_handle as u64),
                ) {
                    Ok(()) => {
                        log::info!("loadModule: module evaluated OK");
                        log::info!("loadModule: paint requested");
                    }
                    Err(e) => log::error!("loadModule: module load failed: {e}"),
                }
            });
            if !posted {
                return Err("tur-host thread has shut down".into());
            }
            Ok(())
        });
    }

    /// Fire one engine wake (the Kotlin `Choreographer` / `Handler` calls this
    /// when due) — posted onto the tur-host thread, which fires the vsync
    /// event and polls the loop (applying the frame's render batch to the
    /// wgpu renderer). Returns `1` if the op was posted.
    ///
    /// Panics raised inside the engine frame tick are caught on the tur-host
    /// thread (rather than unwinding through the engine's `!Send` state).
    /// The panic hook (`logger::init`) has already logged the message + full
    /// backtrace to logcat by the time `catch_unwind` returns; on `Err` we
    /// add a breadcrumb and abort cleanly so the failure stays visible and
    /// the engine never resumes a half-finished frame.
    pub fn pump(handle: jlong) -> jint {
        let Some(route) = handle_to_instance(handle) else {
            return 0;
        };
        let id = route.id;
        let posted = route.host.post(move |state| {
            // On-device crash-diagnostics test, gated by an Android system
            // property so it never fires in normal use and needs no rebuild to
            // toggle:
            //   adb shell setprop debug.tur.crash 1   → panics on next pump
            //   adb shell setprop debug.tur.crash ""  → disabled
            if crate::system_prop("debug.tur.crash").as_deref() == Some("1") {
                // A nested call stack so the captured backtrace is non-trivial
                // and exercises real engine paths when verifying readability.
                #[inline(never)]
                #[track_caller]
                fn panic_from_nested_call(msg: &str) {
                    panic!("{msg}");
                }
                panic_from_nested_call("tur-android panic-hook backtrace test (debug.tur.crash=1)");
            }
            log::trace!("pump: firing vsync + polling loop (tur-host thread)");
            if let Some(instance) = state.instance(id) {
                instance.vsync.fire_vsync();
                instance.pump_loop();
            }
        });
        if posted { 1 } else { 0 }
    }

    /// `pumpMessages(handle): int` — poll the loop WITHOUT firing a vsync,
    /// posted onto the tur-host thread. Kotlin's `FrameLoop.requestPump()`
    /// (a coalesced main-Handler post) calls this when the engine's
    /// worker→host messages or host-loop tasks need processing but no
    /// display frame was requested (`FrameOutcome.schedule == Idle`).
    /// Keeping this separate from [`pump`] (which fires a vsync) is what
    /// lets an idle instance park at 0% CPU instead of ping-ponging at
    /// display refresh rate. (The primary idle wake path is now the
    /// direct host-thread post from the loop waker — this JNI path remains
    /// as the Choreographer-less fallback.)
    pub fn pump_messages(handle: jlong) -> jint {
        let Some(route) = handle_to_instance(handle) else {
            return 0;
        };
        let id = route.id;
        let posted = route.host.post(move |state| {
            log::trace!("pumpMessages: polling loop (tur-host thread)");
            if let Some(instance) = state.instance(id) {
                instance.pump_loop();
            }
        });
        if posted { 1 } else { 0 }
    }

    /// Resize the surface. Resizes the host-side renderer directly AND
    /// forwards the shell `Resize` event to the worker for layout (single
    /// call — see `TurApp::resize`). Posted to the tur-host thread (the
    /// renderer's owning thread). (v1 keeps the original wgpu surface
    /// for the instance lifetime; full surface re-attach with a renderer
    /// swap is a follow-up.)
    pub fn resize(env: &mut JNIEnv, handle: jlong, width: jint, height: jint, dpr: jdouble) {
        catch_void(env, "resize", |_env| {
            let route = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let id = route.id;
            route.host.post(move |state| {
                if let Some(instance) = state.instance(id) {
                    instance
                        .app
                        .resize(width.max(1) as u32, height.max(1) as u32, dpr.max(1.0));
                }
            });
            Ok(())
        });
    }

    /// Push a pointer event. `action` matches Android `MotionEvent.ACTION_*`
    /// constants: 0=DOWN, 1=UP, 2=MOVE, 3=CANCEL. We translate to engine
    /// `PointerInput` with `PointerDeviceKind::Touch`. Posted to the
    /// tur-host thread.
    pub fn push_pointer(
        env: &mut JNIEnv,
        handle: jlong,
        action: jint,
        x: jdouble,
        y: jdouble,
        time_ms: jlong,
    ) {
        catch_void(env, "pushPointer", |_env| {
            let route = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let id = route.id;
            route.host.post(move |state| {
                let Some(instance) = state.instance(id) else {
                    return;
                };
                let device = PointerDeviceKind::Touch;
                let position = Offset::new(x, y);
                let button = MouseButton::Left;
                let ev = match action {
                    0 => PointerInput::PointerDown {
                        position,
                        button,
                        time_ms: time_ms as u64,
                        device,
                    },
                    1 => PointerInput::PointerUp {
                        position,
                        button,
                        device,
                        time_ms: time_ms as u64,
                    },
                    2 => PointerInput::PointerMove {
                        position,
                        device,
                        time_ms: time_ms as u64,
                    },
                    3 => PointerInput::PointerCancel { device },
                    _ => return,
                };
                instance.app.push_platform_event(ShellEvent::Pointer(ev));
            });
            Ok(())
        });
    }

    /// Push a key event. `action`: 0=DOWN, 1=UP. `key`/`code` are browser-style
    /// strings (the Kotlin side maps Android `KeyEvent.keyCode` → these).
    /// Posted to the tur-host thread.
    pub fn push_key(
        env: &mut JNIEnv,
        handle: jlong,
        key: JString,
        code: JString,
        action: jint,
        ctrl: jni::sys::jboolean,
        shift: jni::sys::jboolean,
        alt: jni::sys::jboolean,
        meta: jni::sys::jboolean,
    ) {
        catch_void(env, "pushKey", |env| {
            let route = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let key: String = env.get_string(&key)?.into();
            let code: String = env.get_string(&code)?.into();
            let id = route.id;
            route.host.post(move |state| {
                let Some(instance) = state.instance(id) else {
                    return;
                };
                let event_type = if action == 1 {
                    KeyEventType::Up
                } else {
                    KeyEventType::Down
                };
                instance.app.push_platform_event(ShellEvent::Key(KeyEvent {
                    key,
                    code,
                    modifiers: Modifiers {
                        ctrl: ctrl != 0,
                        shift: shift != 0,
                        alt: alt != 0,
                        meta: meta != 0,
                    },
                    event_type,
                }));
            });
            Ok(())
        });
    }

    /// Escape hatch for embedders: run `f` with `&TurApp` for the given
    /// instance handle. Used from an embedder's *own* JNI trampolines (its
    /// cdylib) to reach plugin-installed per-instance data — e.g.
    /// `with_app(h, |app| EventBus::of(app))` — or to nudge a wake:
    /// `with_app(h, |app| app.request_frame())`.
    ///
    /// Not part of [`standard_jni_exports!`](crate::standard_jni_exports!)
    /// (Kotlin can't pass a Rust closure); the embedder wires its own
    /// `Java_<pkg>_<Class>_*` trampoline that calls this. Returns `None` if
    /// `handle` is `0` or stale (already `destroy`'d) or the instance's
    /// build failed, in which case `f` is not run. Blocking round-trip
    /// onto the tur-host thread — `f` and its return value must be `Send`
    /// (they cross back to the calling thread).
    pub fn with_app<R: Send + 'static>(
        handle: jlong,
        f: impl FnOnce(&TurApp) -> R + Send + 'static,
    ) -> Option<R> {
        let route = handle_to_instance(handle)?;
        let id = route.id;
        route
            .host
            .call(move |state| state.instance(id).map(|i| f(&i.app)))
            .ok()
            .flatten()
    }

    /// Escape hatch mirroring [`with_app`] for the **runtime** handle: run
    /// `f` with `&`[`AndroidRuntime`]. Used from an embedder's own JNI
    /// trampolines — the motivating case is registering an APK asset as a
    /// module source entirely on the Rust side (read via `AAssetManager`,
    /// then `with_runtime(h, |rt| rt.module_sources.register(source))`),
    /// so the JS bundle never crosses the JNI boundary (the registry is
    /// Arc-shared, so that specific case needs no round-trip — this hatch
    /// exposes the rest of the runtime). Returns `None` if `handle` is
    /// `0` or stale, in which case `f` is not run. Blocking round-trip
    /// onto the tur-host thread — `f` and its return value must be `Send`.
    pub fn with_runtime<R: Send + 'static>(
        handle: jlong,
        f: impl FnOnce(&AndroidRuntime) -> R + Send + 'static,
    ) -> Option<R> {
        let route = handle_to_runtime(handle)?;
        route
            .host
            .call(move |state| state.runtime().map(|rt| f(rt)))
            .ok()
            .flatten()
    }

    /// Push an IME composition event onto the platform-event queue. `kind`:
    /// `0=CompositionStart`, `1=CompositionUpdate { text }`,
    /// `2=CompositionEnd { text }`. Routed to the focused editable's
    /// `on_ime_event` by the `ImeSubsystem`. Used by the embedder's
    /// `InputConnection` to deliver multi-char commits / composing text that
    /// can't be represented as a single key event. Posted to the tur-host
    /// thread.
    pub fn push_ime(env: &mut JNIEnv, handle: jlong, kind: jint, text: JString) {
        catch_void(env, "pushIme", |env| {
            let route = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let text: String = env.get_string(&text)?.into();
            let id = route.id;
            route.host.post(move |state| {
                let Some(instance) = state.instance(id) else {
                    return;
                };
                let ime = match kind {
                    0 => ShellEvent::Ime(ImeEvent::CompositionStart),
                    1 => ShellEvent::Ime(ImeEvent::CompositionUpdate { text, cursor: None }),
                    _ => ShellEvent::Ime(ImeEvent::CompositionEnd { text }),
                };
                instance.app.push_platform_event(ime);
            });
            Ok(())
        });
    }

    /// Drop an instance. The route box is reclaimed on the calling thread;
    /// the `AndroidInstance` (and its `Rc<TurApp>`, renderer, surface,
    /// etc.) is removed + dropped on the tur-host thread. The parent
    /// runtime is unaffected and may spawn more instances.
    pub fn destroy(handle: jlong) {
        if handle == 0 {
            return;
        }
        // SAFETY: the handle is a valid `Box<InstanceRoute>` for the
        // instance's lifetime (created by createInstance, freed here).
        let route = unsafe { Box::from_raw(handle as *mut InstanceRoute) };
        let id = route.id;
        let _ = route.host.post(move |state| {
            if state.remove_instance(id).is_some() {
                log::info!("destroy: instance {id} dropped (tur-host thread)");
            }
        });
    }

    /// Drop the runtime. Should be called after all its instances are
    /// destroyed (leftovers are dropped defensively). Tears the tur-host
    /// thread's state down ON that thread, stops its loop, and joins it
    /// before returning — after this returns, every native resource the
    /// runtime owned is gone. Stale instance routes that outlive this call
    /// are harmless: their posts find a closed queue and no-op.
    pub fn destroy_runtime(handle: jlong) {
        if handle == 0 {
            return;
        }
        // SAFETY: the handle is a valid `Box<RuntimeRoute>` for the runtime's
        // lifetime (created by createRuntime, freed here). Take what we need
        // and drop the box BEFORE posting the shutdown op — its HostHandle
        // holds a sender clone, and while `Flow::Stop` makes termination
        // independent of sender liveness, releasing it here keeps the
        // shutdown window tight.
        let (host, join) = {
            let mut route = unsafe { Box::from_raw(handle as *mut RuntimeRoute) };
            (route.host.clone(), route.join.take())
        };
        let _ = host.post_flow(move |state| {
            state.clear_all();
            crate::host_thread::Flow::Stop
        });
        // `join` returning means the stop op ran (state cleared) AND the
        // thread function returned — the full teardown barrier.
        if let Some(join) = join {
            let _ = join.join();
        }
        log::info!("destroyRuntime: tur-host thread stopped + joined");
    }

    fn handle_to_instance(handle: jlong) -> Option<&'static InstanceRoute> {
        if handle == 0 {
            return None;
        }
        // SAFETY: the handle is a valid `Box<InstanceRoute>` for the
        // instance's lifetime (created by createInstance, freed by destroy).
        unsafe { (&*(handle as *const InstanceRoute)).into() }
    }

    fn handle_to_runtime(handle: jlong) -> Option<&'static RuntimeRoute> {
        if handle == 0 {
            return None;
        }
        // SAFETY: the handle is a valid `Box<RuntimeRoute>` for the runtime's
        // lifetime (created by createRuntime, freed by destroyRuntime).
        unsafe { (&*(handle as *const RuntimeRoute)).into() }
    }

    /// Record the process `JavaVM` on the first JNI call. The clipboard backend
    /// and the loop driver attach the frame thread via this. Held in a `Box` so
    /// the address is stable for `'static` borrows (JavaVM is not `Clone`).
    fn stash_java_vm(env: &JNIEnv) -> Result<(), Box<dyn std::error::Error>> {
        if crate::JAVA_VM.get().is_none() {
            let vm = env.get_java_vm()?;
            let _ = crate::JAVA_VM.set(Box::new(vm));
        }
        Ok(())
    }

    /// Run `f(env)` and, on error, throw a Java `RuntimeException` with the
    /// message. Returns the result on success, or `0` on error (for
    /// `jlong`-returning fns). The closure receives `&mut JNIEnv` so it can
    /// make JNI calls without re-borrowing.
    fn catch_into_zero<F>(env: &mut JNIEnv, name: &str, f: F) -> jlong
    where
        F: FnOnce(&mut JNIEnv) -> Result<jlong, Box<dyn std::error::Error>>,
    {
        match f(env) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("{name} failed: {e}");
                let _ = env.throw_new("java/lang/RuntimeException", format!("{name}: {e}"));
                0
            }
        }
    }

    /// Run `f(env)` and, on error, throw a Java `RuntimeException` with the
    /// message. For void-returning fns.
    fn catch_void<F>(env: &mut JNIEnv, name: &str, f: F)
    where
        F: FnOnce(&mut JNIEnv) -> Result<(), Box<dyn std::error::Error>>,
    {
        if let Err(e) = f(env) {
            tracing::error!("{name} failed: {e}");
            let _ = env.throw_new("java/lang/RuntimeException", format!("{name}: {e}"));
        }
    }
}

/// Generate the standard engine-operation JNI entry points inside the caller's
/// cdylib. Emits the instance/runtime-operation `Java_org_tur_TurNative_*`
/// trampolines (`createInstance`, `createHeadlessInstance`,
/// `registerModuleSource`, `releaseModuleSource`, `loadModule`, `pump`,
/// `resize`, `pushPointer`, `pushKey`, `pushIme`, `destroy`,
/// `destroyRuntime`) that forward to [`ops`](crate::ops).
/// Invoking this macro is all an embedder needs to make its `.so` drivable by
/// the Kotlin `org.tur.TurNative` bridge — **runtime creation** is NOT included
/// (it varies per embedder; write your own `Java_<pkg>_<Class>_createRuntime`
/// that calls [`ops::create_runtime`](crate::ops::create_runtime)). The
/// `createInstance` / `createHeadlessInstance` trampolines generated here pass
/// `|b| b` as the `configure_instance` hook — embedders that need build-time
/// per-instance data (via [`TurAppBuilder::instance_data`]) write their own
/// `Java_<pkg>_<Class>_createInstance` instead, mirroring `createRuntime`.
///
/// Invoke under `#[cfg(target_os = "android")]` (the trampolines reference
/// android-only impls):
///
/// ```no_run
/// #[cfg(target_os = "android")]
/// tur_android::standard_jni_exports!();
/// ```
#[macro_export]
macro_rules! standard_jni_exports {
    () => {
        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_createInstance(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            runtime_handle: $crate::jlong,
            surface: $crate::JObject,
            width: $crate::jint,
            height: $crate::jint,
            dpr: $crate::jdouble,
            frame_loop: $crate::JObject,
        ) -> $crate::jlong {
            $crate::ops::create_instance(
                &mut env,
                runtime_handle,
                surface,
                width,
                height,
                dpr,
                frame_loop,
                |b| b,
            )
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_createHeadlessInstance(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            runtime_handle: $crate::jlong,
            frame_loop: $crate::JObject,
        ) -> $crate::jlong {
            $crate::ops::create_headless_instance(&mut env, runtime_handle, frame_loop, |b| b)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_registerModuleSource(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            runtime_handle: $crate::jlong,
            js: $crate::JString,
        ) -> $crate::jlong {
            $crate::ops::register_module_source(&mut env, runtime_handle, js)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_releaseModuleSource(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            runtime_handle: $crate::jlong,
            source_handle: $crate::jlong,
        ) {
            $crate::ops::release_module_source(&mut env, runtime_handle, source_handle)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_loadModule(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
            source_handle: $crate::jlong,
        ) {
            $crate::ops::load_module(&mut env, handle, source_handle)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_pump(
            _env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
        ) -> $crate::jint {
            $crate::ops::pump(handle)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_pumpMessages(
            _env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
        ) -> $crate::jint {
            $crate::ops::pump_messages(handle)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_resize(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
            width: $crate::jint,
            height: $crate::jint,
            dpr: $crate::jdouble,
        ) {
            $crate::ops::resize(&mut env, handle, width, height, dpr)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_pushPointer(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
            action: $crate::jint,
            x: $crate::jdouble,
            y: $crate::jdouble,
            time_ms: $crate::jlong,
        ) {
            $crate::ops::push_pointer(&mut env, handle, action, x, y, time_ms)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_pushKey(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
            key: $crate::JString,
            code: $crate::JString,
            action: $crate::jint,
            ctrl: $crate::jboolean,
            shift: $crate::jboolean,
            alt: $crate::jboolean,
            meta: $crate::jboolean,
        ) {
            $crate::ops::push_key(&mut env, handle, key, code, action, ctrl, shift, alt, meta)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_pushIme(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
            kind: $crate::jint,
            text: $crate::JString,
        ) {
            $crate::ops::push_ime(&mut env, handle, kind, text)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_destroy(
            _env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
        ) {
            $crate::ops::destroy(handle)
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Java_org_tur_TurNative_destroyRuntime(
            _env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
        ) {
            $crate::ops::destroy_runtime(handle)
        }
    };
}
