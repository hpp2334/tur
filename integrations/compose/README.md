# tur Compose integration

A pure-Kotlin Jetpack Compose library (`org.tur`) for embedding the
[tur](../../) JavaScript rendering engine in Android apps. tur renders into an
Android `Surface` via wgpu/Vulkan; this AAR wraps the engine in a single
`@Composable TurView` that handles the surface lifecycle, touch/key input, the
frame loop, and IME.

**This library ships no native code.** The app links its own `.so` (built from
[`libs/tur-android`](../../libs/tur-android) as an rlib + the app's plugin set)
and hands the resulting engine handle to `TurView` via a `TurEngineFactory`. This
keeps the Kotlin lib reusable across apps with different plugin sets, and
supports multiple independent `TurView`s in one app (tur-as-plugin-system).

## Quick start

### 1. App's native `.so` (Rust)

Create a `cdylib` crate that depends on `tur-android` and adds your plugins. The
`tur_android::standard_jni_exports!()` macro generates the standard engine-op
JNI symbols; you write one `createEngine` fn with your plugin set:

```rust
// your-app/native/src/lib.rs
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

#[cfg(target_os = "android")]
tur_android::standard_jni_exports!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_yourapp_AppNative_createEngine(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
    surface: tur_android::JObject,
    width: tur_android::jint,
    height: tur_android::jint,
    dpr: tur_android::jdouble,
    frame_loop: tur_android::JObject,
) -> tur_android::jlong {
    tur_android::ops::create_with_plugins(
        &mut env, context, surface, width, height, dpr, frame_loop,
        |b| b
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
    external fun createEngine(
        context: Context, surface: Surface, width: Int, height: Int,
        dpr: Double, frameLoop: FrameLoop,
    ): Long
}

// MainActivity.kt
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val js = remember { assets.open("bundle.js").bufferedReader().use { it.readText() } }
            val factory = remember {
                TurEngineFactory { ctx, surface, w, h, dpr, loop ->
                    AppNative.createEngine(ctx, surface, w, h, dpr, loop)
                }
            }
            TurView(js = js, engineFactory = factory, modifier = Modifier.fillMaxSize())
        }
    }
}
```

`js` is an ES module that imports from `tur:std`, `tur:animation`, etc. — the
same bundle the web playground runs.

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

| Module | Plugin | Notes |
|---|---|---|
| `tur:std` | `TurStdPlugin` | Column/Row/Stack, Text, Input, Image, ScrollView, … |
| `tur:animation` | `TurAnimationPlugin` | `Opacity`, `Transform`, `AnimatedContainer`, … |
| `tur:clipboard` | `TurClipboardPlugin` | requires the `Clipboard` capability (Android default: `AndroidClipboard` via JNI to `ClipboardManager`) |
| `tur:net` | `TurNetPlugin` | requires the `Http` capability (default: `NativeHttp` — reqwest + rustls) |

`tur_android::ops::create_with_plugins` pre-registers Android-default
capabilities (`NoopCursor`, `AndroidClipboard`, `NativeHttp`) + the wgpu
renderer, native fonts, and wall-clock — your `configure` callback only adds
plugins (and may override any capability).

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

## Multi-view

Each `TurView` builds its own engine via the factory (isolated JS context +
plugins) and its own `FrameLoop`. `TurNative` is stateless and handle-based, so
the same bridge safely drives any number of engines. Drop multiple `TurView`s
into one Compose UI for a tur-as-plugin-system setup.

## Limitations (v1)

- **Surface re-attach** on `surfaceDestroyed` → `surfaceCreated` tears down and
  rebuilds the engine; a live-edit-friendly renderer swap (preserving JS state)
  is a follow-up.
- **Multi-touch** is single-pointer (the engine's `PlatformEvent::Pointer` is
  single-position). The primary pointer is tracked.
