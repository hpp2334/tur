package org.tur

import android.content.Context
import android.view.Surface

/**
 * Builds a native tur engine over an Android [Surface] and returns its opaque
 * handle.
 *
 * The app implements this (typically as a thin wrapper around its own
 * `external fun createEngine(…)` JNI function exported by its `.so`). The
 * Kotlin lib calls it from inside [TurView]'s `SurfaceHolder.Callback` once the
 * surface is ready, then drives the returned handle via [TurNative]. This is
 * the seam that lets every app build its engine with **its own plugin set**
 * without the Kotlin lib knowing which `.so` is loaded or which plugins are
 * registered.
 *
 * The [FrameLoop] is created by the Kotlin lib and passed in so the engine's
 * native loop driver can arm wake-ups against it (Choreographer / delayed
 * Handler) — the same loop [TurEngine] pumps from.
 *
 * @return the opaque engine handle (a `Long`), or `0` on failure.
 */
fun interface TurEngineFactory {
    fun create(
        context: Context,
        surface: Surface,
        width: Int,
        height: Int,
        dpr: Double,
        frameLoop: FrameLoop,
    ): Long
}
