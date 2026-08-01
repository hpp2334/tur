package org.tur

import android.os.Handler
import android.os.Looper
import android.view.Choreographer

/**
 * Frame scheduler the native `LoopDriver` drives.
 *
 * The engine decides when it wants the next wake-up ([NextFrame] verdict) and
 * calls one of `scheduleVsync` / `scheduleDelayed` / `cancel` back through JNI.
 * When the wake-up fires, [FrameLoop] invokes [onWake] (which [TurInstance]
 * wires to the engine's `pump`), completing the loop:
 *
 * ```
 * engine run_frame() → LoopDriver.request_next(Vsync) → FrameLoop.scheduleVsync()
 *   → Choreographer frame → FrameLoop.onWake() → nativePump() → engine run_frame() → …
 * ```
 *
 * Lives on the main looper (where `SurfaceHolder.Callback` and input dispatch
 * arrive), matching the single-threaded assumption the native side relies on.
 *
 * [onWake] / [onAfterPump] are settable (default `null`) so a [FrameLoop] can be
 * constructed before the instance handle exists and wired up by [TurInstance]
 * afterwards — the runtime needs a `FrameLoop` to hand to native
 * `createInstance`, but the `pump` target only exists once `createInstance`
 * returns.
 */
class FrameLoop {
    private val handler = Handler(Looper.getMainLooper())
    private var frameCallback: Choreographer.FrameCallback? = null
    private var delayedToken: Runnable? = null

    /** Fired when a scheduled wake-up is due. [TurInstance] sets this to `pump`. */
    var onWake: (() -> Unit)? = null

    /**
     * Optional callback fired after [onWake] in each wake-up. The Compose
     * integration sets this to sync the Android soft-keyboard / IME with the
     * engine's focused-element state (poll `focusedIsEditable`, then
     * `showSoftInput` / `hideSoftInput`). Runs on the main looper, same as
     * [onWake]. `null` by default so non-IME embedders (and tests) are
     * unaffected.
     */
    var onAfterPump: (() -> Unit)? = null

    /** Schedule a wake on the next display frame (Android `Choreographer`). */
    fun scheduleVsync() {
        if (frameCallback != null) return // already armed
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                frameCallback = null
                onWake?.invoke()
                onAfterPump?.invoke()
            }
        }
        frameCallback = cb
        Choreographer.getInstance().postFrameCallback(cb)
    }

    /** Schedule a wake [delayMs] milliseconds from now. */
    fun scheduleDelayed(delayMs: Long) {
        if (delayedToken != null) return // already armed (coalesce)
        val r = Runnable {
            delayedToken = null
            onWake?.invoke()
            onAfterPump?.invoke()
        }
        delayedToken = r
        handler.postDelayed(r, delayMs.coerceAtLeast(1))
    }

    /** Cancel any pending wake-up (the engine went idle). */
    fun cancel() {
        frameCallback?.let { Choreographer.getInstance().removeFrameCallback(it) }
        frameCallback = null
        delayedToken?.let { handler.removeCallbacks(it) }
        delayedToken = null
    }
}
