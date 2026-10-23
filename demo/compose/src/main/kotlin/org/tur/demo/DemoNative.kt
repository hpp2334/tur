package org.tur.demo

import android.content.Context
import org.tur.FrameLoop

/**
 * JNI bridge to the demo's native lib (`libtur_demo.so`, built from
 * `demo/compose/native`).
 *
 * This is the app's own glue: it loads the `.so` and declares the entry points
 * that build a tur runtime with the **demo's** plugin set (Std + Animation +
 * Clipboard + Net + the playground's rut compile service + FilePicker over a
 * stub backend) and register the embedded rut playground as a module source.
 * The standard engine-op symbols (`Java_org_tur_TurNative_*` — createInstance /
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
     * Register the **rut playground** embedded in the native lib (the root
     * source — `mod.rut` + the generated case registry — plus the declared
     * `menu` child) on [runtimeHandle]'s registries. Returns the opaque
     * root-source handle (`0` on failure — a `RuntimeException` is also
     * thrown), which `TurView` passes to `TurNative.loadModule`; native
     * resolves it as a multi-file module and boots the whole tree through the
     * engine's multi-file rut rail. Nothing crosses the Kotlin↔Rust boundary
     * as text: the sources are `include_str!`-embedded Rust-side, so the APK
     * ships no module asset.
     */
    external fun createPlaygroundModuleSource(runtimeHandle: Long): Long
}
