# tur Compose integration

A pure-Kotlin Jetpack Compose library (`org.tur`) for embedding the
[tur](../../) JavaScript rendering engine in Android apps. tur renders into an
Android `Surface` via wgpu/Vulkan; this AAR wraps the engine in a single
`@Composable TurView` that handles the surface lifecycle, touch/key input, the
frame loop, and IME.

**This library ships no native code.** The app links its own `.so` (built from
[`libs/tur-android`](../../libs/tur-android) as an rlib + the app's plugin set)
and creates a shared `TurRuntime` once (via `rememberTurRuntime`). From that
runtime, `TurView` spawns isolated instances — each its own JS realm, attached
to a `Surface`. This keeps the Kotlin lib reusable across apps with different
plugin sets, and supports multiple `TurView`s sharing one runtime (isolated JS
state, shared fonts/clock/capabilities — tur-as-plugin-system).

## Quick start

### 1. App's native `.so` (Rust)

Create a `cdylib` crate that depends on `tur-android` and adds your plugins. The
`tur_android::standard_jni_exports!()` macro generates the standard engine-op
JNI symbols (instance creation, pump, input, …); you write one `createRuntime`
fn with your plugin set:

```rust
// your-app/native/src/lib.rs
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

#[cfg(target_os = "android")]
tur_android::standard_jni_exports!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_yourapp_AppNative_createRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
) -> tur_android::jlong {
    tur_android::ops::create_runtime(&mut env, context, |b| b
        .plugin(tur_engine::TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .plugin(tur_engine::TurClipboardPlugin)
        .plugin(tur_net_native::TurNetPlugin)
        // …your custom plugins here…
    )
}
```

See [`demo/compose/native`](../../demo/compose/native) for a complete working
crate — copy it as your template.

### 2. App's Kotlin glue

```kotlin
object AppNative {
    init { System.loadLibrary("your_app") }
    external fun createRuntime(context: Context): Long
}

// MainActivity.kt
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            // Build the shared runtime once (the .so registers your plugins).
            val runtime = rememberTurRuntime { ctx -> AppNative.createRuntime(ctx) }
            // Register the bundle as a module source (auto-released on dispose).
            val source = rememberTurModuleSource(runtime) { rt ->
                rt.registerModuleSource(assets.open("bundle.js").bufferedReader().use { it.readText() })
            }
            // TurView spawns an isolated instance from the runtime per surface
            // and loads the registered source by handle.
            TurView(runtime = runtime, sourceHandle = source, modifier = Modifier.fillMaxSize())
        }
    }
}
```

The source is an ES module that imports from `tur:std`, `tur:animation`, etc. —
the same bundle the web playground runs.

### Module sources: Kotlin string vs Rust-side handle

`TurView` loads by **source handle**, not string. Two ways to get a handle:

- **From Kotlin**: `runtime.registerModuleSource(js)` / `rememberTurModuleSource`
  — the string crosses JNI exactly once, at registration; every (re)load is a
  refcount handoff.
- **From Rust** (zero JNI string traffic): your `.so` reads the source natively
  (e.g. an APK asset via the NDK `AAssetManager`) and registers it on the
  runtime's `ModuleSourceRegistry` — see
  `tur_android::ops::with_runtime(h, |rt| rt.module_sources.register(source))`
  and the demo's `DemoNative.createAssetModuleSource`. Kotlin only ever sees
  the opaque `Long`.

Sources are shared across every instance of the runtime; release them with
`runtime.releaseModuleSource(handle)` (or let `rememberTurModuleSource` do it).
Stale handles are safe misses — ids are monotonic and never reused.

### 3. Dependencies

```kotlin
// app/build.gradle.kts
dependencies {
    implementation(project(":tur-compose")) // or the published AAR
}
```

Plus a cargo-ndk task that builds your `.so` into `src/main/jniLibs/arm64-v8a/`
(see `demo/compose/build.gradle.kts` for the task wiring).

## What you get

The engine ships with every standard plugin available to register from your
`.so`:

| Module | Plugin |
|---|---|
| `tur:std` | `TurStdPlugin` |
| `tur:animation` | `TurAnimationPlugin` |
| `tur:clipboard` | `TurClipboardPlugin` |
| `tur:net` | `TurNetPlugin` |

`tur_android::ops::create_runtime` pre-registers Android-default capabilities
(`AndroidClipboard`, `NativeHttp`) + native fonts and wall-clock
— your `configure` callback only adds plugins.

## Multi-instance

One `TurRuntime` (built once via `rememberTurRuntime`) can spawn any number of
isolated instances. Each `TurView` spawns its own instance from the shared
runtime when its surface becomes ready, and tears it down when the surface is
destroyed (the runtime survives). Multiple `TurView`s sharing one runtime keep
fully isolated JS state while sharing fonts/clock/capabilities/plugins. You can
also spawn **headless** instances via `runtime.createHeadlessInstance()` for
off-screen JS computation.

## Native build prerequisites

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973  # or newer
```

## Supported ABIs

`arm64-v8a` only — real Android devices. The engine crate has no arch-specific
code, so adding `x86_64-linux-android` / `armv7-linux-androideabi` Rust targets
and `abiFilters` entries is straightforward if you need them.

## Limitations (v1)

- **Surface re-attach** on `surfaceDestroyed` → `surfaceCreated` tears down and
  rebuilds the instance; a live-edit-friendly renderer swap (preserving JS state)
  is a follow-up.
- **Multi-touch** is single-pointer (the engine's `ShellEvent::Pointer` is
  single-position). The primary pointer is tracked.
