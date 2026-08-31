package org.tur.demo

import android.content.Context
import org.tur.FrameLoop

/**
 * JNI bridge to the demo's native lib (`libtur_demo.so`, built from
 * `demo/compose/native`).
 *
 * This is the app's own glue: it loads the `.so` and declares the entry points
 * that build a tur runtime with the **demo's** plugin set (Std + Animation +
 * Clipboard + Net + DemoHelper) and create module sources natively. The
 * standard engine-op symbols (`Java_org_tur_TurNative_*` — createInstance /
 * pump / input / IME / module sources / …) come from the same `.so` (generated
 * by `tur_android::standard_jni_exports!`) and are resolved by the Kotlin
 * lib's `org.tur.TurNative`; this object only owns the app-specific exports.
 *
 * The returned `Long`s are opaque handles, passed to [org.tur.TurRuntime] /
 * [org.tur.TurView] (see [MainActivity]).
 */
object DemoNative {
    init {
        System.loadLibrary("tur_demo")
    }

    /**
     * Build the shared runtime with the demo plugins. Returns an opaque runtime
     * handle (`0` on failure — a `RuntimeException` is also thrown from native).
     * No surface — instances are created via `TurNative.createInstance` and
     * attached to a surface via `TurNative.attachInstance`.
     */
    external fun createRuntime(context: Context): Long

    /**
     * Read the APK asset at [path] **on the Rust side** (NDK AAssetManager)
     * and register it as a module source on [runtimeHandle]'s registry.
     * Returns an opaque source handle (`0` on failure — a
     * `RuntimeException` is also thrown).
     *
     * The bundle never crosses the Kotlin↔Rust boundary as a string: the
     * bytes are read natively, registered as a shared `Arc<str>`, and
     * `TurView` loads it into an instance by handle.
     */
    external fun createAssetModuleSource(
        runtimeHandle: Long,
        path: String,
        assets: android.content.res.AssetManager,
    ): Long
}
