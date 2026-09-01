package org.tur

import android.content.Context

/**
 * Builds a native tur runtime (the shared, created-once substrate) and returns
 * its opaque handle.
 *
 * The app implements this (typically as a thin wrapper around its own
 * `external fun createRuntime(…)` JNI function exported by its `.so`). The
 * Kotlin lib calls it once (usually via [rememberTurRuntime]) to obtain the
 * runtime handle, then spawns isolated [TurInstance]s from it via
 * [TurRuntime.createInstance]. This is
 * the seam that lets every app build its runtime with **its own plugin set**
 * without the Kotlin lib knowing which `.so` is loaded or which plugins are
 * registered.
 *
 * @return the opaque runtime handle (a `Long`), or `0` on failure.
 */
fun interface TurRuntimeFactory {
    fun create(context: Context): Long
}
