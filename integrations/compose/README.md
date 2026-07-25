# tur Compose integration

A Jetpack Compose library for embedding the [tur](../../) JavaScript rendering
engine in Android apps. tur renders into an Android `Surface` via wgpu/Vulkan;
this AAR wraps the engine in a single `@Composable TurView` that handles the
surface lifecycle, touch/key input, and the frame loop.

## Quick start

```kotlin
// app/build.gradle.kts
dependencies {
    implementation(project(":tur-compose")) // or the published AAR
}

// MainActivity.kt
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val js = remember {
                assets.open("playground.js").bufferedReader().use { it.readText() }
            }
            TurView(js = js, modifier = Modifier.fillMaxSize())
        }
    }
}
```

`js` is an ES module that imports from `tur:std`, `tur:animation`, etc. — the
same bundle the web playground runs. See [`demo/compose`](../../demo/compose)
for a complete working app.

## What you get

The engine ships with every standard plugin registered:

| Module | Plugin | Notes |
|---|---|---|
| `tur:std` | `TurStdPlugin` | Column/Row/Stack, Text, Input, Image, ScrollView, … |
| `tur:animation` | `TurAnimationPlugin` | `Opacity`, `Transform`, `AnimatedContainer`, … |
| `tur:clipboard` | `TurClipboardPlugin` | `AndroidClipboard` via JNI to `ClipboardManager` |
| `tur:net` | `TurNetPlugin` | `NativeHttp` (reqwest + rustls) |
| `tur-ext/demo-helper` | `TurDemoPlugin` | swc TS transpilation (compiler fns only; `pickFile`/`saveFile` are no-op stubs) |

Plus a bundled Roboto + Roboto Mono font loader (fontique has no Android
system-font backend) and a `NoopCursor` (touch device).

## Native build

The `libtur_android.so` is cross-compiled from [`libs/tur-android`](../../libs/tur-android)
via `cargo-ndk`. The AAR's `build.gradle.kts` wires a `buildTurNative` /
`copyTurNative` task pair that runs `cargo ndk build` and drops the `.so` into
`src/main/jniLibs/arm64-v8a/` before `preBuild`, so a plain
`./gradlew assembleDebug` produces a working AAR with no manual NDK steps
beyond having the NDK installed.

Prerequisites:

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973  # or newer
```

## Supported ABIs

`arm64-v8a` only — real Android devices. The engine crate has no
arch-specific code, so adding `x86_64-linux-android` / `armv7-linux-androideabi`
Rust targets and `abiFilters` entries is straightforward if you need them,
but shipping both ABIs triples the APK size for no benefit on consumer
hardware.

## Limitations (v1)

- **IME composition** (CJK text entry) is not yet wired — basic key dispatch
  works, but `compositionStart`/`Update`/`End` events from the Android
  `InputConnection` are not forwarded. Paste works (via the clipboard backend).
- **Surface re-attach** on `surfaceDestroyed` → `surfaceCreated` tears down and
  rebuilds the engine; a live-edit-friendly renderer swap (preserving JS state)
  is a follow-up.
- **Multi-touch** is single-pointer (the engine's `PlatformEvent::Pointer` is
  single-position). The primary pointer is tracked.
