package org.tur

import android.content.Context
import android.view.Surface
import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/**
 * Owns one native tur engine instance.
 *
 * Built on top of [TurNative] (the JNI bridge): holds the engine `handle`,
 * drives frames via a [FrameLoop], and translates Android input into the
 * engine's platform-event stream. Construction is synchronous — [create]
 * blocks on wgpu adapter/device acquisition (done on the main looper, which is
 * the same thread `SurfaceHolder.Callback.surfaceCreated` arrives on).
 *
 * Use [TurView] in Compose rather than this class directly — [TurView] wires
 * surface lifecycle, input dispatch, and the frame loop together.
 */
class TurEngine private constructor(
    private val frameLoop: FrameLoop,
    /** Atomic so the finalizer / close() race is safe even though we're single-threaded. */
    private val handleCell: AtomicLong,
) : Closeable {

    private val handle: Long get() = handleCell.get()

    companion object {
        /**
         * Build the engine over [surface]. Blocks on wgpu setup (adapter +
         * device + surface configuration). Throws if the GPU is unavailable or
         * surface creation fails.
         *
         * The [FrameLoop] wake callback is bound to [pump] for the instance
         * returned, so the engine starts advancing as soon as JS is loaded.
         */
        fun create(
            context: Context,
            surface: Surface,
            width: Int,
            height: Int,
            dpr: Double,
        ): TurEngine {
            // Holder so the FrameLoop's wake lambda can read the handle before
            // nativeCreate has returned it (the lambda captures the cell, not
            // the value).
            val handleCell = AtomicLong(0L)
            val loop = FrameLoop(onWake = {
                val h = handleCell.get()
                if (h != 0L) TurNative.pump(h)
            })
            val handle = TurNative.create(context, surface, width, height, dpr, loop)
            require(handle != 0L) { "nativeCreate returned 0 (see logcat for the cause)" }
            handleCell.set(handle)
            return TurEngine(loop, handleCell)
        }
    }

    /** Evaluate [js] (an ES module) and request a paint. */
    fun loadModule(js: String) {
        check(handle != 0L) { "engine destroyed" }
        TurNative.loadModule(handle, js)
    }

    /** Push a new logical size + dpr (from `SurfaceHolder.Callback.surfaceChanged`). */
    fun resize(width: Int, height: Int, dpr: Double) {
        if (handle == 0L) return
        TurNative.resize(handle, width, height, dpr)
    }

    /** Fire one engine wake (the Choreographer / Handler callback). */
    fun pump() {
        if (handle == 0L) return
        TurNative.pump(handle)
    }

    /** Dispatch a pointer (touch) event. [action] is `MotionEvent.ACTION_*`. */
    fun pushPointer(action: Int, x: Double, y: Double, timeMs: Long) {
        if (handle == 0L) return
        TurNative.pushPointer(handle, action, x, y, timeMs)
    }

    /** Dispatch a key event (browser-style `key`/`code`). */
    fun pushKey(
        key: String,
        code: String,
        action: Int,
        ctrl: Boolean,
        shift: Boolean,
        alt: Boolean,
        meta: Boolean,
    ) {
        if (handle == 0L) return
        TurNative.pushKey(handle, key, code, action, ctrl, shift, alt, meta)
    }

    /** Whether the focused element is an editable text field. */
    fun focusedIsEditable(): Boolean =
        handle != 0L && TurNative.focusedIsEditable(handle)

    /**
     * Push an IME composition event. [kind]: `0=Start`, `1=Update`, `2=End`.
     */
    fun pushIme(kind: Int, text: String) {
        if (handle == 0L) return
        TurNative.pushIme(handle, kind, text)
    }

    /**
     * Install a callback fired after each engine pump (on the main looper,
     * immediately after the frame runs). The Compose integration uses it to
     * sync the soft keyboard with the engine's focused-element state. Pass
     * `null` to clear.
     */
    fun setAfterPump(cb: (() -> Unit)?) {
        frameLoop.onAfterPump = cb
    }

    /** The opaque native handle (for advanced embedders / debugging). */
    fun nativeHandle(): Long = handle

    /** Drop the engine and free native resources. Idempotent. */
    override fun close() {
        val h = handleCell.getAndSet(0L)
        if (h == 0L) return
        frameLoop.cancel()
        TurNative.destroy(h)
    }

    protected fun finalize() {
        close()
    }
}
