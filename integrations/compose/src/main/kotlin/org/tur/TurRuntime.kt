package org.tur

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur runtime — the shared, created-once substrate that
 * isolated [TurInstance]s are spawned from.
 *
 * Built on top of [TurNative] (the JNI bridge): holds the runtime handle
 * (fonts, clock, capabilities, plugins, wgpu instance). The app builds the
 * runtime via a [TurRuntimeFactory] (which calls into its own `.so`) and hands
 * the resulting handle here.
 *
 * From a runtime, create rendering instances via [createInstance] (attached to
 * a [android.view.Surface]) or headless instances via [createHeadlessInstance].
 * Multiple instances share the runtime's fonts/clock/capabilities/plugins while
 * keeping fully isolated JS state.
 *
 * @param handle the opaque native runtime pointer returned by the app's
 *   `createRuntime` JNI function. `0` is treated as "destroyed".
 */
class TurRuntime(
    handle: Long,
) : Closeable {

    private val handleCell: AtomicLong = AtomicLong(handle)
    /** The opaque native handle (for advanced embedders / passing to [TurNative]). */
    val handle: Long get() = handleCell.get()

    /**
     * Register a JS module source on the runtime's shared registry and return
     * its opaque handle. Load it into any instance of this runtime via
     * [TurInstance.loadModule]. Pair with [releaseModuleSource] (or use
     * [rememberTurModuleSource], which releases automatically).
     *
     * Sources created on the Rust side (e.g. an APK asset read natively) can
     * be registered from Rust and passed here as a raw handle — no JNI string
     * crossing at all.
     */
    fun registerModuleSource(js: String): Long {
        check(handle != 0L) { "runtime destroyed" }
        return TurNative.registerModuleSource(handle, js)
    }

    /** Drop a registered module source. Idempotent; safe after [close]. */
    fun releaseModuleSource(sourceHandle: Long) {
        if (handle == 0L || sourceHandle == 0L) return
        TurNative.releaseModuleSource(handle, sourceHandle)
    }

    /**
     * Spawn an isolated rendering instance attached to [surface] and return it.
     * Shares this runtime's fonts/clock/capabilities/plugins; gets its own JS
     * realm, element tree, and renderer.
     */
    fun createInstance(
        surface: android.view.Surface,
        width: Int,
        height: Int,
        dpr: Double,
    ): TurInstance {
        check(handle != 0L) { "runtime destroyed" }
        val frameLoop = FrameLoop()
        val h = TurNative.createInstance(handle, surface, width, height, dpr, frameLoop)
        check(h != 0L) { "createInstance returned 0 (see logcat)" }
        return TurInstance(h, frameLoop)
    }

    /**
     * Spawn an isolated headless instance (no surface, no rendering). Runs JS +
     * capabilities + events only. Useful for off-screen JS computation that
     * shares a runtime with visible views.
     */
    fun createHeadlessInstance(): TurInstance {
        check(handle != 0L) { "runtime destroyed" }
        val frameLoop = FrameLoop()
        val h = TurNative.createHeadlessInstance(handle, frameLoop)
        check(h != 0L) { "createHeadlessInstance returned 0 (see logcat)" }
        return TurInstance(h, frameLoop)
    }

    /** Drop the runtime and free native resources. Destroy all instances first. Idempotent. */
    override fun close() {
        val h = handleCell.getAndSet(0L)
        if (h == 0L) return
        TurNative.destroyRuntime(h)
    }

    protected fun finalize() {
        close()
    }
}
