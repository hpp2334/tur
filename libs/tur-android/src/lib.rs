//! tur Android JNI runtime — the native side of the Compose integration.
//!
//! Exposes a C ABI (under the `Java_org_tur_TurNative_*` JNI names) that Kotlin
//! (`integrations/compose`) calls to build, drive, and tear down a tur engine
//! instance. The engine itself, the renderer, and all plugins come from
//! `tur-engine` + `tur-animation` + plugins unchanged; this crate is
//! only the embedder glue (surface, events, loop driver) — the same three
//! integration seams the wasm and native harnesses use.
//!
//! On non-Android targets the crate compiles as a stub (the JNI entry points
//! are gated), so the workspace still builds on desktop for `cargo check`.

// On non-Android targets the whole crate is an unreachable stub: the JNI
// entry points (the only callers of `init_logger_once`, `AndroidApp::create`,
// the surface helpers, etc.) are `cfg(target_os = "android")`-gated, so the
// bodies would otherwise be flagged as dead code. Allow it.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

mod app;
mod module_source;
pub mod scheduler;
mod surface;

pub use module_source::ModuleSourceRegistry;

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
///   **runtime**, then load into any of its instances by handle). (Focused-
///   element state is pushed to Kotlin via `FrameLoop.onFocusChanged` from
///   the engine's focus-change handler — no `focused_is_editable` JNI poll.)
/// - [`destroy_runtime`] drops the runtime.
///
/// Runtime creation varies per embedder (plugin set), so [`create_runtime`]
/// takes a `configure` closure and is called from the embedder's own
/// `Java_<pkg>_<Class>_createRuntime` JNI function. The instance/runtime ops
/// are standard and generated by [`standard_jni_exports!`].
#[cfg(target_os = "android")]
pub mod ops {
    use std::ffi::c_void;

    use jni::JNIEnv;
    use jni::objects::{JObject, JString};
    use jni::sys::{jdouble, jint, jlong};
    use tur_engine::core::layout::{MouseButton, Offset};
    use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
    use tur_engine::core::platform::{ImeEvent, PlatformEvent, PointerDeviceKind, PointerInput};
    use tur_engine::{TurApp, TurAppBuilder, TurRuntimeBuilder};

    use crate::app::{AndroidInstance, AndroidRuntime};

    /// `createRuntime(env, context): long`
    ///
    /// Builds the shared runtime (no surface) and returns an opaque pointer
    /// handle (boxed `AndroidRuntime`) Kotlin holds as a `long`. Called from
    /// the embedder's own `Java_<pkg>_<Class>_createRuntime` JNI function,
    /// which passes a `configure` closure that adds the embedder's plugin set.
    ///
    /// Returns `0` on failure (a `RuntimeException` is also thrown).
    pub fn create_runtime(
        env: &mut JNIEnv,
        context: JObject,
        configure: impl FnOnce(TurRuntimeBuilder) -> TurRuntimeBuilder,
    ) -> jlong {
        catch_into_zero(env, "createRuntime", |env| {
            crate::init_logger_once();
            stash_java_vm(env)?;
            let context_ref = env.new_global_ref(context)?;
            log::info!("createRuntime: building shared runtime");
            let runtime = AndroidRuntime::build(context_ref, configure)?;
            log::info!("createRuntime: runtime built OK");
            let boxed = Box::new(runtime);
            Ok(Box::into_raw(boxed) as jlong)
        })
    }

    /// `createInstance(env, runtimeHandle, surface, width, height, dpr, frameLoop): long`
    ///
    /// Spawns an isolated rendering instance attached to the given Android
    /// `Surface`, sharing the runtime's fonts/clock/capabilities/wgpu-instance.
    /// Returns an instance handle (boxed `AndroidInstance`).
    ///
    /// `configure_instance` receives the [`TurAppBuilder`] before the
    /// surface-backed renderer is attached — chain
    /// [`TurAppBuilder::instance_data`] on it and return it to stamp
    /// per-instance data at build time. The standard
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
        configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a> + 'static,
    ) -> jlong {
        catch_into_zero(env, "createInstance", |env| {
            let runtime = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            let surface_ref = env.new_global_ref(surface)?;
            let frame_loop_ref = env.new_global_ref(frame_loop)?;
            // Acquire the ANativeWindow* from the Surface.
            let env_ptr = env.get_raw();
            let surface_ptr = surface_ref.as_raw();
            let anw = unsafe {
                crate::surface::native_window_from_surface(
                    env_ptr as *mut c_void,
                    surface_ptr as *mut c_void,
                )
            };
            if anw.is_null() {
                return Err("ANativeWindow_fromSurface returned null".into());
            }
            let window_handle = unsafe { crate::surface::AndroidWindowHandle::new(anw) };
            let frame_loop_handle = crate::scheduler::FrameLoopRef::new(frame_loop_ref);
            log::info!(
                "createInstance: building instance ({}x{} @{}x)",
                width,
                height,
                dpr
            );
            let instance = pollster::block_on(AndroidInstance::build_with_surface(
                runtime,
                runtime.default_worker_pool.clone(),
                &runtime.tokio_handle(),
                &runtime.wgpu_instance,
                window_handle,
                width.max(1) as u32,
                height.max(1) as u32,
                dpr.max(1.0),
                frame_loop_handle,
                configure_instance,
            ))?;
            log::info!("createInstance: instance built OK");
            let boxed = Box::new(instance);
            Ok(Box::into_raw(boxed) as jlong)
        })
    }

    /// `createHeadlessInstance(env, runtimeHandle, frameLoop): long`
    ///
    /// Spawns an isolated headless instance (no surface, no rendering) from
    /// the runtime. Returns an instance handle.
    ///
    /// `configure_instance` mirrors [`create_instance`]'s hook — chain
    /// [`TurAppBuilder::instance_data`] on the builder and return it.
    pub fn create_headless_instance(
        env: &mut JNIEnv,
        runtime_handle: jlong,
        frame_loop: JObject,
        configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a> + 'static,
    ) -> jlong {
        catch_into_zero(env, "createHeadlessInstance", |env| {
            let runtime = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            let frame_loop_ref = env.new_global_ref(frame_loop)?;
            let frame_loop_handle = crate::scheduler::FrameLoopRef::new(frame_loop_ref);
            log::info!("createHeadlessInstance: building headless instance");
            let instance = AndroidInstance::build_headless(
                runtime,
                runtime.default_worker_pool.clone(),
                &runtime.tokio_handle(),
                frame_loop_handle,
                configure_instance,
            )?;
            log::info!("createHeadlessInstance: instance built OK");
            let boxed = Box::new(instance);
            Ok(Box::into_raw(boxed) as jlong)
        })
    }

    /// `registerModuleSource(env, runtimeHandle, js): long`
    ///
    /// Register a module source on the runtime's shared
    /// [`ModuleSourceRegistry`](crate::ModuleSourceRegistry) and return its
    /// opaque handle (`0` on failure). The source crosses JNI exactly once,
    /// here — `loadModule` then loads it into any instance of the runtime by
    /// handle. Rust embedders skip even this hop: read the source natively
    /// and call `runtime.module_sources.register(…)` via [`with_runtime`].
    pub fn register_module_source(env: &mut JNIEnv, runtime_handle: jlong, js: JString) -> jlong {
        catch_into_zero(env, "registerModuleSource", |env| {
            let runtime = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            let js: String = env.get_string(&js)?.into();
            Ok(runtime.module_sources.register(js) as jlong)
        })
    }

    /// `releaseModuleSource(env, runtimeHandle, sourceHandle)`
    ///
    /// Drop a registered module source. Idempotent — an unknown/stale handle
    /// is a no-op (handles are monotonic ids, never reused). Everything left
    /// registered is released wholesale when the runtime is destroyed.
    pub fn release_module_source(env: &mut JNIEnv, runtime_handle: jlong, source_handle: jlong) {
        catch_void(env, "releaseModuleSource", |_env| {
            let runtime = handle_to_runtime(runtime_handle).ok_or("invalid runtime handle")?;
            runtime.module_sources.remove(source_handle as u64);
            Ok(())
        })
    }

    /// Evaluate the registered module source `source_handle` as an ES module
    /// (resolved by the engine's `TurModuleLoader` — `tur:std`,
    /// `tur:animation`, etc. must already be registered, which instance
    /// creation does), then request a paint so the bundle renders on the
    /// next frame.
    ///
    /// The registry's `Arc<str>` flows to the worker by refcount — no copy,
    /// no JNI string traffic. A source produced on the Rust side (e.g. an
    /// APK asset read via `AAssetManager`) therefore reaches the JS realm
    /// without ever being serialized across the JNI boundary.
    pub fn load_module(env: &mut JNIEnv, handle: jlong, source_handle: jlong) {
        catch_void(env, "loadModule", |_env| {
            let instance = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let source = instance
                .module_sources
                .get(source_handle as u64)
                .ok_or("unknown module source handle")?;
            log::info!(
                "loadModule: source handle {source_handle} ({} bytes)",
                source.len()
            );
            futures::executor::block_on(instance.app.load_module(source))?;
            log::info!("loadModule: module evaluated OK");
            log::info!("loadModule: paint requested");
            Ok(())
        });
    }

    /// Fire one engine wake (the Kotlin `Choreographer` / `Handler` calls this
    /// when due). Returns `1` on success.
    ///
    /// Panics raised inside the engine frame tick are caught here (rather than
    /// letting them unwind across the `extern "system"` JNI boundary, which
    /// aborts via `panic_cannot_unwind` → an opaque SIGABRT whose tombstone
    /// shows only the panic machinery, not the panicking call site). The panic
    /// hook (`logger::init`) has already logged the message + full backtrace to
    /// logcat by the time `catch_unwind` returns; on `Err` we add a breadcrumb
    /// and abort cleanly so the failure stays visible and the engine never
    /// resumes a half-finished frame.
    pub fn pump(handle: jlong) -> jint {
        let Some(instance) = handle_to_instance(handle) else {
            return 0;
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
            log::trace!("pump: firing vsync + polling loop");
            instance.vsync.fire_vsync();
            instance.pump_loop();
        }));
        match result {
            Ok(()) => 1,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<non-string panic payload>".into());
                log::error!("pump: panic caught at JNI boundary, aborting: {msg}");
                std::process::abort();
            }
        }
    }

    /// `pumpMessages(env omitted, handle): int` — poll the main loop
    /// WITHOUT firing a vsync. The Kotlin `FrameLoop.requestPump()` (a
    /// coalesced main-Handler post) calls this when the engine's
    /// worker→main messages or main-loop tasks need processing but no
    /// display frame was requested (`FrameOutcome.schedule == Idle`).
    /// Keeping this separate from [`pump`] (which fires a vsync) is what
    /// lets an idle instance park at 0% CPU instead of ping-ponging at
    /// display refresh rate.
    pub fn pump_messages(handle: jlong) -> jint {
        let Some(instance) = handle_to_instance(handle) else {
            return 0;
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            instance.pump_loop();
        }));
        match result {
            Ok(()) => 1,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<non-string panic payload>".into());
                log::error!("pumpMessages: panic caught at JNI boundary, aborting: {msg}");
                std::process::abort();
            }
        }
    }

    /// Resize the surface. Resizes the host-side renderer directly AND
    /// forwards `PlatformEvent::Resize` to the worker for layout (single
    /// call — see `TurApp::resize`). (v1 keeps the original wgpu surface
    /// for the instance lifetime; full surface re-attach with a renderer
    /// swap is a follow-up.)
    pub fn resize(env: &mut JNIEnv, handle: jlong, width: jint, height: jint, dpr: jdouble) {
        catch_void(env, "resize", |_env| {
            let instance = handle_to_instance(handle).ok_or("invalid instance handle")?;
            instance
                .app
                .resize(width.max(1) as u32, height.max(1) as u32, dpr.max(1.0));
            Ok(())
        });
    }

    /// Push a pointer event. `action` matches Android `MotionEvent.ACTION_*`
    /// constants: 0=DOWN, 1=UP, 2=MOVE, 3=CANCEL. We translate to engine
    /// `PointerInput` with `PointerDeviceKind::Touch`.
    pub fn push_pointer(
        env: &mut JNIEnv,
        handle: jlong,
        action: jint,
        x: jdouble,
        y: jdouble,
        time_ms: jlong,
    ) {
        catch_void(env, "pushPointer", |_env| {
            let instance = handle_to_instance(handle).ok_or("invalid instance handle")?;
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
                _ => return Ok(()),
            };
            instance.app.push_platform_event(PlatformEvent::Pointer(ev));
            Ok(())
        });
    }

    /// Push a key event. `action`: 0=DOWN, 1=UP. `key`/`code` are browser-style
    /// strings (the Kotlin side maps Android `KeyEvent.keyCode` → these).
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
            let instance = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let key: String = env.get_string(&key)?.into();
            let code: String = env.get_string(&code)?.into();
            let event_type = if action == 1 {
                KeyEventType::Up
            } else {
                KeyEventType::Down
            };
            instance
                .app
                .push_platform_event(PlatformEvent::Key(KeyEvent {
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
    /// `handle` is `0` or stale (already `destroy`'d), in which case `f` is
    /// not run.
    ///
    /// Must be called on the instance's own thread — `Rc<TurApp>` and the
    /// underlying boa `Context` are `!Send` / `!Sync` (same constraint as
    /// every other op here). Mirrors `TurApp::with_element` /
    /// `TurApp::with_boa_context` in the engine.
    pub fn with_app<R>(handle: jlong, f: impl FnOnce(&TurApp) -> R) -> Option<R> {
        let instance = handle_to_instance(handle)?;
        Some(f(&instance.app))
    }

    /// Escape hatch mirroring [`with_app`] for the **runtime** handle: run
    /// `f` with `&`[`AndroidRuntime`]. Used from an embedder's own JNI
    /// trampolines — the motivating case is registering an APK asset as a
    /// module source entirely on the Rust side (read via `AAssetManager`,
    /// then `with_runtime(h, |rt| rt.module_sources.register(source))`),
    /// so the JS bundle never crosses the JNI boundary. Returns `None` if
    /// `handle` is `0` or stale, in which case `f` is not run.
    ///
    /// Unlike [`with_app`] there is no `!Send` constraint — the runtime
    /// handle is process-shared infrastructure — but follow the crate's JNI
    /// discipline and call it on the main thread.
    pub fn with_runtime<R>(handle: jlong, f: impl FnOnce(&AndroidRuntime) -> R) -> Option<R> {
        let runtime = handle_to_runtime(handle)?;
        Some(f(runtime))
    }

    /// Push an IME composition event onto the platform-event queue. `kind`:
    /// `0=CompositionStart`, `1=CompositionUpdate { text }`,
    /// `2=CompositionEnd { text }`. Routed to the focused editable's
    /// `on_ime_event` by the `ImeSubsystem`. Used by the embedder's
    /// `InputConnection` to deliver multi-char commits / composing text that
    /// can't be represented as a single key event.
    pub fn push_ime(env: &mut JNIEnv, handle: jlong, kind: jint, text: JString) {
        catch_void(env, "pushIme", |env| {
            let instance = handle_to_instance(handle).ok_or("invalid instance handle")?;
            let text: String = env.get_string(&text)?.into();
            let ime = match kind {
                0 => PlatformEvent::Ime(ImeEvent::CompositionStart),
                1 => PlatformEvent::Ime(ImeEvent::CompositionUpdate { text, cursor: None }),
                _ => PlatformEvent::Ime(ImeEvent::CompositionEnd { text }),
            };
            instance.app.push_platform_event(ime);
            Ok(())
        });
    }

    /// Drop an instance. The boxed `AndroidInstance` is reclaimed; its
    /// `Rc<TurApp>` (and the renderer, surface, etc.) drop in turn. The
    /// parent runtime is unaffected and may spawn more instances.
    pub fn destroy(handle: jlong) {
        if handle == 0 {
            return;
        }
        unsafe {
            let _ = Box::from_raw(handle as *mut AndroidInstance);
        }
    }

    /// Drop the runtime. Should be called after all its instances are
    /// destroyed. The boxed `AndroidRuntime` is reclaimed.
    pub fn destroy_runtime(handle: jlong) {
        if handle == 0 {
            return;
        }
        unsafe {
            let _ = Box::from_raw(handle as *mut AndroidRuntime);
        }
    }

    fn handle_to_instance(handle: jlong) -> Option<&'static AndroidInstance> {
        if handle == 0 {
            return None;
        }
        // SAFETY: the handle is a valid `Box<AndroidInstance>` for the
        // instance's lifetime (created by createInstance, freed by destroy).
        unsafe { (&*(handle as *const AndroidInstance)).into() }
    }

    fn handle_to_runtime(handle: jlong) -> Option<&'static AndroidRuntime> {
        if handle == 0 {
            return None;
        }
        // SAFETY: the handle is a valid `Box<AndroidRuntime>` for the
        // runtime's lifetime (created by createRuntime, freed by
        // destroyRuntime).
        unsafe { (&*(handle as *const AndroidRuntime)).into() }
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
