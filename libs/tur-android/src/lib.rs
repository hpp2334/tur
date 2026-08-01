//! tur Android JNI runtime — the native side of the Compose integration.
//!
//! Exposes a C ABI (under the `Java_org_tur_TurNative_*` JNI names) that Kotlin
//! (`integrations/compose`) calls to build, drive, and tear down a tur engine
//! instance. The engine itself, the renderer, and all plugins come from
//! `tur-engine` + `tur-animation` + `tur-demo-plugin` unchanged; this crate is
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
mod loop_driver;
mod surface;

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
/// drives (load JS, pump a frame, push input, query/edit IME, tear down).
///
/// Every function here is a plain `pub fn` (not a JNI entry point). The
/// `standard_jni_exports!()` macro below generates the `#[unsafe(no_mangle)]`
/// `Java_org_tur_TurNative_*` trampolines that forward to these, expanded
/// inside the embedder's cdylib so the symbols are local (guaranteed
/// link-retained, never stripped by GC-sections).
///
/// The one operation NOT here is engine creation — that varies per embedder
/// (plugin set), so it lives in [`create_with_plugins`] and is called from the
/// embedder's own `Java_…_createEngine` JNI function.
#[cfg(target_os = "android")]
pub mod ops {
    use std::ffi::c_void;

    use jni::JNIEnv;
    use jni::objects::{JObject, JString};
    use jni::sys::{jdouble, jint, jlong};
    use tur_engine::TurEngineBuilder;
    use tur_engine::core::layout::{MouseButton, Offset};
    use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
    use tur_engine::core::platform::{ImeEvent, PlatformEvent, PointerDeviceKind, PointerInput};

    use crate::app::AndroidApp;

    /// `createEngine(env, context, surface, width, height, dpr, frameLoop): long`
    ///
    /// Builds the engine over the Android `Surface` and returns an opaque
    /// pointer handle (boxed `AndroidApp`) Kotlin holds as a `long`. Called from
    /// the embedder's own `Java_<pkg>_<Class>_createEngine` JNI function, which
    /// passes a `configure` closure that adds the embedder's plugin set. This
    /// wrapper does all the shared Android boilerplate (logger, JavaVM stash,
    /// ANativeWindow acquisition, wgpu surface) before delegating to
    /// [`AndroidApp::build`].
    ///
    /// Returns `0` on failure (a `RuntimeException` is also thrown).
    pub fn create_with_plugins(
        env: &mut JNIEnv,
        context: JObject,
        surface: JObject,
        width: jint,
        height: jint,
        dpr: jdouble,
        frame_loop: JObject,
        configure: impl FnOnce(TurEngineBuilder) -> TurEngineBuilder,
    ) -> jlong {
        catch_into_zero(env, "create", |env| {
            crate::init_logger_once();
            stash_java_vm(env)?;
            let context_ref = env.new_global_ref(context)?;
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
            let frame_loop_handle = crate::loop_driver::FrameLoopRef::new(frame_loop_ref);
            log::info!("create: building engine ({}x{} @{}x)", width, height, dpr);
            let app = AndroidApp::build(
                context_ref,
                window_handle,
                width.max(1) as u32,
                height.max(1) as u32,
                dpr.max(1.0),
                frame_loop_handle,
                configure,
            )?;
            log::info!("create: engine built OK");
            let boxed = Box::new(app);
            Ok(Box::into_raw(boxed) as jlong)
        })
    }

    /// Evaluate `js` as an ES module (resolved by the engine's `TurModuleLoader`
    /// — `tur:std`, `tur:animation`, etc. must already be registered, which
    /// `createEngine` does). Then request a paint so the bundle renders on the
    /// next frame.
    pub fn load_module(env: &mut JNIEnv, handle: jlong, js: JString) {
        catch_void(env, "loadModule", |env| {
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            let js: String = env.get_string(&js)?.into();
            log::info!("loadModule: {} bytes", js.len());
            app.app.load_module(&js)?;
            log::info!("loadModule: module evaluated OK");
            app.app.request_paint();
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
        let Some(app) = handle_to_app(handle) else {
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
            log::trace!("pump: firing wake");
            app.loop_driver.fire();
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

    /// Push a `Resize` event reflecting the new surface dimensions. (v1 keeps
    /// the original wgpu surface for the engine lifetime; full surface re-attach
    /// with a renderer swap is a follow-up.)
    pub fn resize(env: &mut JNIEnv, handle: jlong, width: jint, height: jint, dpr: jdouble) {
        catch_void(env, "resize", |_env| {
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            app.app.push_platform_event(PlatformEvent::Resize {
                logical_width: width.max(1) as u32,
                logical_height: height.max(1) as u32,
                dpr: dpr.max(1.0),
            });
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
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
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
            app.app.push_platform_event(PlatformEvent::Pointer(ev));
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
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            let key: String = env.get_string(&key)?.into();
            let code: String = env.get_string(&code)?.into();
            let event_type = if action == 1 {
                KeyEventType::Up
            } else {
                KeyEventType::Down
            };
            app.app.push_platform_event(PlatformEvent::Key(KeyEvent {
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

    /// True if the currently-focused element is an editable text field. The
    /// embedder polls this after each pump to decide whether to raise the soft
    /// keyboard (and `hideSoftInput` when it flips back to false).
    pub fn focused_is_editable(handle: jlong) -> jni::sys::jboolean {
        let Some(app) = handle_to_app(handle) else {
            return 0;
        };
        if app.app.focused_is_editable() { 1 } else { 0 }
    }

    /// Push an IME composition event onto the platform-event queue. `kind`:
    /// `0=CompositionStart`, `1=CompositionUpdate { text }`,
    /// `2=CompositionEnd { text }`. Routed to the focused editable's
    /// `on_ime_event` by the `ImeSubsystem`. Used by the embedder's
    /// `InputConnection` to deliver multi-char commits / composing text that
    /// can't be represented as a single key event.
    pub fn push_ime(env: &mut JNIEnv, handle: jlong, kind: jint, text: JString) {
        catch_void(env, "pushIme", |env| {
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            let text: String = env.get_string(&text)?.into();
            let ime = match kind {
                0 => PlatformEvent::Ime(ImeEvent::CompositionStart),
                1 => PlatformEvent::Ime(ImeEvent::CompositionUpdate { text, cursor: None }),
                _ => PlatformEvent::Ime(ImeEvent::CompositionEnd { text }),
            };
            app.app.push_platform_event(ime);
            Ok(())
        });
    }

    /// Drop the engine. The boxed `AndroidApp` is reclaimed; its `Rc<TurApp>`
    /// (and the renderer, surface, etc.) drop in turn.
    pub fn destroy(handle: jlong) {
        if handle == 0 {
            return;
        }
        unsafe {
            let _ = Box::from_raw(handle as *mut AndroidApp);
        }
    }

    fn handle_to_app(handle: jlong) -> Option<&'static AndroidApp> {
        if handle == 0 {
            return None;
        }
        // SAFETY: the handle is a valid `Box<AndroidApp>` for the engine's
        // lifetime (created by createEngine, freed by destroy). We return
        // a `'static` reference because the JNI layer guarantees single-threaded
        // access (Android main thread) and the box outlives every call until
        // destroy.
        unsafe { (&*(handle as *const AndroidApp)).into() }
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
/// cdylib. Emits the eight `Java_org_tur_TurNative_*` trampolines
/// (`loadModule`, `pump`, `resize`, `pushPointer`, `pushKey`,
/// `focusedIsEditable`, `pushIme`, `destroy`) that forward to
/// [`ops`](crate::ops). Invoking this macro is all an embedder needs to make
/// its `.so` drivable by the Kotlin `org.tur.TurNative` bridge — engine
/// **creation** is NOT included (it varies per embedder; write your own
/// `Java_<pkg>_<Class>_createEngine` that calls
/// [`ops::create_with_plugins`](crate::ops::create_with_plugins)).
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
        pub extern "system" fn Java_org_tur_TurNative_loadModule(
            mut env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
            js: $crate::JString,
        ) {
            $crate::ops::load_module(&mut env, handle, js)
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
        pub extern "system" fn Java_org_tur_TurNative_focusedIsEditable(
            _env: $crate::JNIEnv,
            _class: $crate::JClass,
            handle: $crate::jlong,
        ) -> $crate::jboolean {
            $crate::ops::focused_is_editable(handle)
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
    };
}
