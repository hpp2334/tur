package ai.tur

import android.content.Context
import android.view.Surface

/**
 * JNI bridge to the native tur engine (`libtur_android.so`).
 *
 * Every method is a thin `external fun` over the `Java_ai_tur_TurNative_*`
 * entry points exported by `libs/tur-android`. The Kotlin side ([TurView] /
 * [TurEngine]) owns the lifecycle: [create] on surface-ready, [loadModule] to
 * run JS, [pump] each Choreographer frame, [destroy] on teardown.
 *
 * The `handle` returned by [create] is an opaque pointer (the boxed
 * `AndroidApp` on the Rust side) passed back to every other call. `0` means
 * creation failed (a `RuntimeException` is also thrown from native).
 */
object TurNative {
    init {
        System.loadLibrary("tur_android")
    }

    /** Build the engine over [surface]. Returns an opaque engine handle. */
    external fun create(
        context: Context,
        surface: Surface,
        width: Int,
        height: Int,
        dpr: Double,
        frameLoop: FrameLoop,
    ): Long

    /** Evaluate [js] as an ES module (`import … from "tur:*"` resolved by the engine). */
    external fun loadModule(handle: Long, js: String)

    /** Fire one engine wake — call each Choreographer / Handler tick. */
    external fun pump(handle: Long): Int

    /** Push a new surface size (logical px + dpr). */
    external fun resize(handle: Long, width: Int, height: Int, dpr: Double)

    /**
     * Push a pointer event. [action] matches `MotionEvent.ACTION_*`:
     * `0=DOWN`, `1=UP`, `2=MOVE`, `3=CANCEL`. Coordinates are logical px
     * relative to the surface; [timeMs] is `SystemClock.uptimeMillis()`.
     */
    external fun pushPointer(handle: Long, action: Int, x: Double, y: Double, timeMs: Long)

    /**
     * Push a key event. [action] is `0=DOWN`, `1=UP`. [key]/[code] are
     * browser-style strings (see [InputMapper] for the Android→browser map).
     */
    external fun pushKey(
        handle: Long,
        key: String,
        code: String,
        action: Int,
        ctrl: Boolean,
        shift: Boolean,
        alt: Boolean,
        meta: Boolean,
    )

    /** Drop the engine and free its resources. */
    external fun destroy(handle: Long)
}
