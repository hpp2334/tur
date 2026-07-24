//! tur Android JNI runtime — the native side of the Compose integration.
//!
//! Exposes a C ABI (under the `Java_ai_tur_TurNative_*` JNI names) that Kotlin
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
mod fonts;
mod loop_driver;
mod surface;

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

#[cfg(target_os = "android")]
mod exports {
    use std::ffi::c_void;

    use jni::objects::{JClass, JObject, JString};
    use jni::sys::{jdouble, jint, jlong};
    use jni::JNIEnv;
    use tur_engine::core::layout::{MouseButton, Offset};
    use tur_engine::core::platform::key_event::{KeyEvent, KeyEventType, Modifiers};
    use tur_engine::core::platform::{PointerDeviceKind, PointerInput, PlatformEvent};

    use crate::app::AndroidApp;

    /// `TurNative.nativeCreate(env, context, surface, width, height, dpr, frameLoop): long`
    ///
    /// Builds the engine over the Android `Surface` and returns an opaque
    /// pointer handle (boxed `AndroidApp`) Kotlin holds as a `long`.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_create(
        mut env: JNIEnv,
        _class: JClass,
        context: JObject,
        surface: JObject,
        width: jint,
        height: jint,
        dpr: jdouble,
        frame_loop: JObject,
    ) -> jlong {
        catch_into_zero(&mut env, "create", |env| {
            crate::init_logger_once();
            stash_java_vm(&env)?;
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
            let app = AndroidApp::create(
                context_ref,
                window_handle,
                width.max(1) as u32,
                height.max(1) as u32,
                dpr.max(1.0),
                frame_loop_handle,
            )?;
            log::info!("create: engine built OK");
            let boxed = Box::new(app);
            Ok(Box::into_raw(boxed) as jlong)
        })
    }

    /// `TurNative.nativeLoadModule(env, handle, js): void`
    ///
    /// Evaluate `js` as an ES module (resolved by the engine's `TurModuleLoader`
    /// — `tur:std`, `tur:animation`, etc. must already be registered, which
    /// `nativeCreate` does). Then request a paint so the bundle renders on the
    /// next frame.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_loadModule(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        js: JString,
    ) {
        catch_void(&mut env, "loadModule", |env| {
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

    /// `TurNative.pump(env, handle): int`
    ///
    /// Fire one engine wake (the Kotlin `Choreographer` / `Handler` calls this
    /// when due). Returns `1`.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_pump(
        _unused_env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jint {
        let Some(app) = handle_to_app(handle) else {
            return 0;
        };
        log::trace!("pump: firing wake");
        app.loop_driver.fire();
        1
    }

    /// `TurNative.nativeResize(env, handle, width, height, dpr): void`
    ///
    /// Push a `Resize` event reflecting the new surface dimensions. (v1 keeps
    /// the original wgpu surface for the engine lifetime; full surface re-attach
    /// with a renderer swap is a follow-up.)
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_resize(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        width: jint,
        height: jint,
        dpr: jdouble,
    ) {
        catch_void(&mut env, "nativeResize", |_env| {
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            app.app.push_platform_event(PlatformEvent::Resize {
                logical_width: width.max(1) as u32,
                logical_height: height.max(1) as u32,
                dpr: dpr.max(1.0),
            });
            Ok(())
        });
    }

    /// `TurNative.nativePushPointer(env, handle, action, x, y, timeMs): void`
    ///
    /// `action` matches Android `MotionEvent.ACTION_*` constants: 0=DOWN, 1=UP,
    /// 2=MOVE, 3=CANCEL. We translate to engine `PointerInput` with
    /// `PointerDeviceKind::Touch`.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_pushPointer(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        action: jint,
        x: jdouble,
        y: jdouble,
        time_ms: jlong,
    ) {
        catch_void(&mut env, "nativePushPointer", |_env| {
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            let device = PointerDeviceKind::Touch;
            let position = Offset::new(x, y);
            let button = MouseButton::Left;
            let ev = match action {
                0 => PointerInput::PointerDown { position, button, time_ms: time_ms as u64, device },
                1 => PointerInput::PointerUp { position, button, device, time_ms: time_ms as u64 },
                2 => PointerInput::PointerMove { position, device, time_ms: time_ms as u64 },
                3 => PointerInput::PointerCancel { device },
                _ => return Ok(()),
            };
            app.app.push_platform_event(PlatformEvent::Pointer(ev));
            Ok(())
        });
    }

    /// `TurNative.nativePushKey(env, handle, key, code, action, ctrl, shift, alt, meta): void`
    ///
    /// `action`: 0=DOWN, 1=UP. `key`/`code` are browser-style strings (the
    /// Kotlin side maps Android `KeyEvent.keyCode` → these).
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_pushKey(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        key: JString,
        code: JString,
        action: jint,
        ctrl: jni::sys::jboolean,
        shift: jni::sys::jboolean,
        alt: jni::sys::jboolean,
        meta: jni::sys::jboolean,
    ) {
        catch_void(&mut env, "nativePushKey", |env| {
            let app = handle_to_app(handle).ok_or("invalid engine handle")?;
            let key: String = env.get_string(&key)?.into();
            let code: String = env.get_string(&code)?.into();
            let event_type = if action == 1 { KeyEventType::Up } else { KeyEventType::Down };
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

    /// `TurNative.nativeDestroy(env, handle): void`
    ///
    /// Drop the engine. The boxed `AndroidApp` is reclaimed; its `Rc<TurApp>`
    /// (and the renderer, surface, etc.) drop in turn.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_ai_tur_TurNative_destroy(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
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
        // lifetime (created by nativeCreate, freed by nativeDestroy). We return
        // a `'static` reference because the JNI layer guarantees single-threaded
        // access (Android main thread) and the box outlives every call until
        // nativeDestroy.
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
    /// message. For void-returning JNI fns.
    fn catch_void<F>(env: &mut JNIEnv, name: &str, f: F)
    where
        F: FnOnce(&mut JNIEnv) -> Result<(), Box<dyn std::error::Error>>,
    {
        if let Err(e) = f(env) {
            tracing::error!("{name} failed: {e}");
            let _ = env.throw_new("java/lang/RuntimeException", format!("{name}: {e}"));
        }
    }

    // Referenced by `nativeCreate`'s global-ref paths; keep the import live even
    // though the refs are moved into the engine immediately.
}
