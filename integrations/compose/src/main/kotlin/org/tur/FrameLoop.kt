package org.tur

import android.os.Handler
import android.os.Looper
import android.view.Choreographer

/**
 * Frame scheduler the native `LoopDriver` drives.
 *
 * The engine decides when it wants the next wake-up ([NextFrame] verdict) and
 * calls one of `scheduleVsync` / `scheduleDelayed` / `cancel` back through JNI.
 * When the wake-up fires, [FrameLoop] invokes [onWake] (which the [TurEngine]
 * wires to `nativePump`), completing the loop:
 *
 * ```
 * engine run_frame() → LoopDriver.request_next(Vsync) → FrameLoop.scheduleVsync()
 *   → Choreographer frame → FrameLoop.onWake() → nativePump() → engine run_frame() → …
 * ```
 *
 * Lives on the main looper (where `SurfaceHolder.Callback` and input dispatch
 * arrive), matching the single-threaded assumption the native side relies on.
 */
class FrameLoop(private val onWake: () -> Unit) {
    private val handler = Handler(Looper.getMainLooper())
    private var frameCallback: Choreographer.FrameCallback? = null
    private var delayedToken: Runnable? = null

    /** Schedule a wake on the next display frame (Android `Choreographer`). */
    fun scheduleVsync() {
        if (frameCallback != null) return // already armed
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                frameCallback = null
                onWake()
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
            onWake()
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
