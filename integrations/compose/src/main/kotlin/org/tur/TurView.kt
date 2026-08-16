package org.tur

import android.content.Context
import android.text.InputType
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView

/**
 * A Compose surface that spawns an isolated tur instance from [runtime] and
 * renders the given JS into it.
 *
 * The single-call integration point: obtain a [TurRuntime] once (e.g. via
 * [rememberTurRuntime]), drop this composable into any Compose UI, pass a
 * module **source handle** (an ES module source registered on the runtime via
 * [TurRuntime.registerModuleSource] or created on the Rust side), and tur
 * renders into the surface. When the surface becomes ready the view spawns an
 * instance via [TurRuntime.createInstance]; when the surface is destroyed the
 * instance is torn down (the runtime survives). Pointer (touch), resize, and
 * the frame loop are wired automatically; basic key dispatch is wired when
 * the surface has focus.
 *
 * Multiple `TurView`s sharing one [runtime] coexist as isolated instances
 * (each its own JS realm) while sharing fonts/clock/capabilities/plugins — the
 * basis for a tur-as-plugin-system setup.
 *
 * Example:
 * ```
 * val runtime = rememberTurRuntime { ctx -> DemoNative.createRuntime(ctx) }
 * val source = rememberTurModuleSource(runtime) {
 *     runtime.registerModuleSource(assets.open("playground.js").bufferedReader().use { it.readText() })
 * }
 * TurView(runtime = runtime, sourceHandle = source, modifier = Modifier.fillMaxSize())
 * ```
 *
 * @param runtime the shared [TurRuntime] to spawn the instance from.
 * @param sourceHandle a registered module-source handle (from
 *   [TurRuntime.registerModuleSource] or a Rust-side registration). The
 *   source is an ES module importing from `tur:std` / `tur:animation` /
 *   etc., resolved by the engine's module loader. Loading by handle means
 *   the bundle never crosses the Kotlin↔Rust boundary as a string per load.
 * @param dpr force a DPR (defaults to the window's `Resources.displayMetrics.density`).
 */
@Composable
fun TurView(
    runtime: TurRuntime,
    sourceHandle: Long,
    modifier: Modifier = Modifier,
    dpr: Double? = null,
) {
    val context = LocalContext.current
    val resolvedDpr = dpr ?: context.resources.displayMetrics.density.toDouble()

    val surfaceView = remember { TurSurfaceView(context) }

    AndroidView(
        factory = { surfaceView },
        modifier = modifier.fillMaxSize(),
    )

    DisposableEffect(surfaceView) {
        surfaceView.bind(runtime, sourceHandle, resolvedDpr)
        onDispose { surfaceView.unbind() }
    }
}

/**
 * Create a [TurRuntime] once via [factory] and remember it across recomposition,
 * disposing (destroying) it when [factory]'s key changes or the composable
 * leaves the composition.
 *
 * The typical entry point: the app loads its `.so` and supplies a factory that
 * calls its `createRuntime` JNI function with its plugin set.
 */
@Composable
fun rememberTurRuntime(
    factory: (Context) -> Long,
): TurRuntime {
    val context = LocalContext.current
    val runtime = remember { TurRuntime(factory(context)) }
    DisposableEffect(runtime) {
        onDispose { runtime.close() }
    }
    return runtime
}

/**
 * Create a module-source handle once via [factory] and remember it across
 * recomposition, releasing it ([TurRuntime.releaseModuleSource]) when
 * [runtime] changes or the composable leaves the composition.
 *
 * The [factory] receives the [TurRuntime] (whose handle Rust-side
 * registrations were made against) and returns a source handle — either from
 * [TurRuntime.registerModuleSource] or, when the Rust side created the source
 * (e.g. reading an APK asset natively via the embedder's `.so`), the raw
 * handle it returned. Returning `0` signals "no source" and skips release.
 */
@Composable
fun rememberTurModuleSource(
    runtime: TurRuntime,
    factory: (TurRuntime) -> Long,
): Long {
    val sourceHandle = remember(runtime) { factory(runtime) }
    DisposableEffect(runtime, sourceHandle) {
        onDispose { runtime.releaseModuleSource(sourceHandle) }
    }
    return sourceHandle
}

/**
 * `SurfaceView` subclass that owns the [TurInstance] lifecycle + input dispatch.
 *
 * The instance is created lazily via [bind] (called once the surface is ready —
 * see [TurView]'s `DisposableEffect`). All methods must be called on the main
 * looper (where `SurfaceHolder.Callback` and input dispatch arrive).
 */
private class TurSurfaceView(context: android.content.Context) : SurfaceView(context) {
    private var instance: TurInstance? = null
    private var pendingSourceHandle: Long = 0L
    private var dprValue: Double = 0.0
    private var runtime: TurRuntime? = null
    /** Tracks the last IME state we drove so we only call the IMM on
     *  show↔hide transitions (not every frame). */
    private var imeActive = false
    /** True once the user has touched the surface. We suppress `showSoftInput`
     *  until then so a programmatically-focused editable (e.g. the editor
     *  auto-focusing on launch) doesn't pop the keyboard unprompted — the
     *  keyboard should only appear in response to a user tap, per standard
     *  Android UX. */
    private var userInteracted = false

    init {
        // SurfaceView renders on its own layer below the view hierarchy by
        // default; the Compose host window's opaque background would cover it.
        // Put this surface on top so the tur-rendered content is visible, and
        // use RGBA_8888 so the engine's clear color shows directly.
        setZOrderOnTop(true)
        holder.setFormat(android.graphics.PixelFormat.RGBA_8888)
    }

    /** Stash the source handle + dpr + runtime and register the surface
     *  callback; spawn the instance when the surface is ready. */
    fun bind(runtime: TurRuntime, sourceHandle: Long, dpr: Double) {
        pendingSourceHandle = sourceHandle
        dprValue = dpr
        this.runtime = runtime
        isFocusable = true
        isFocusableInTouchMode = true
        requestFocus()
        holder.addCallback(surfaceCallback)
        setOnTouchListener { _, event ->
            userInteracted = true
            val inst = instance ?: run {
                return@setOnTouchListener false
            }
            // `MotionEvent.getX/Y` are in physical px (Android's view coord
            // space); the engine hit-tests in logical px, so divide by dpr to
            // land taps in the same space as the layout.
            val dpr = dprValue.coerceAtLeast(1.0)
            inst.pushPointer(
                event.actionMasked,
                event.x.toDouble() / dpr,
                event.y.toDouble() / dpr,
                event.eventTime,
            )
            true
        }
    }

    /** Tear down: remove callbacks + destroy the instance (runtime survives). */
    fun unbind() {
        holder.removeCallback(surfaceCallback)
        setOnTouchListener(null)
        instance?.setAfterPump(null)
        instance?.close()
        instance = null
        imeActive = false
    }

    private val surfaceCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (instance != null) return
            val sourceHandle = pendingSourceHandle
            val rt = runtime ?: return
            if (sourceHandle == 0L) return
            // `SurfaceHolder.surfaceFrame` (and `surfaceChanged`'s width/height)
            // report *physical* pixels, but the engine's `viewportSize$` (and
            // thus JS-side layout thresholds like the playground's
            // `isMobile$ = width < 720`) is in *logical / CSS* pixels. Divide
            // by the density to convert; the renderer re-applies `dpr` when
            // scaling the scene to the physical surface. Without this, a 1440px-
            // wide phone at density 3.5 reports a 1440px logical width and the
            // playground renders its cramped desktop 3-pane layout instead of
            // the mobile tab-bar layout.
            val dpr = dprValue.coerceAtLeast(1.0)
            val w = (holder.surfaceFrame.width() / dpr).toInt().coerceAtLeast(1)
            val h = (holder.surfaceFrame.height() / dpr).toInt().coerceAtLeast(1)
            instance = try {
                // Spawn an isolated instance from the runtime (which calls
                // TurNative.createInstance under the hood — the runtime's loop
                // driver arms wake-ups against the instance's FrameLoop).
                rt.createInstance(holder.surface, w, h, dprValue).also {
                    it.loadModule(sourceHandle)
                    // After each frame, sync the soft keyboard with the
                    // engine's focused-element state (reads the value native
                    // pushed into the FrameLoop via onFocusChanged).
                    it.setAfterPump { syncIme() }
                }
            } catch (e: Throwable) {
                android.util.Log.e("TurView", "instance create failed", e)
                null
            }
        }

        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            // `width`/`height` here are physical px (same unit as
            // `surfaceFrame`); convert to logical px before pushing to the
            // engine. See `surfaceCreated` for the unit rationale.
            val dpr = dprValue.coerceAtLeast(1.0)
            instance?.resize(
                (width / dpr).toInt().coerceAtLeast(1),
                (height / dpr).toInt().coerceAtLeast(1),
                dprValue,
            )
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {
            instance?.close()
            instance = null
        }
    }

    override fun onKeyDown(keyCode: Int, event: android.view.KeyEvent): Boolean {
        val inst = instance ?: return super.onKeyDown(keyCode, event)
        val mapped = InputMapper.map(keyCode) ?: return super.onKeyDown(keyCode, event)
        inst.pushKey(
            key = mapped.first,
            code = mapped.second,
            action = 0, // DOWN
            ctrl = event.isCtrlPressed,
            shift = event.isShiftPressed,
            alt = event.isAltPressed,
            meta = event.isMetaPressed,
        )
        return true
    }

    override fun onKeyUp(keyCode: Int, event: android.view.KeyEvent): Boolean {
        val inst = instance ?: return super.onKeyUp(keyCode, event)
        val mapped = InputMapper.map(keyCode) ?: return super.onKeyUp(keyCode, event)
        inst.pushKey(
            key = mapped.first,
            code = mapped.second,
            action = 1, // UP
            ctrl = event.isCtrlPressed,
            shift = event.isShiftPressed,
            alt = event.isAltPressed,
            meta = event.isMetaPressed,
        )
        return true
    }

    // --- Soft keyboard / IME ------------------------------------------------
    //
    // The engine renders its own caret, so focus + the visible cursor work
    // without any platform IME. The missing piece is raising the soft
    // keyboard and routing its text back. We declare the surface a text editor
    // and supply a minimal `InputConnection` that turns IME commits into
    // engine events. The engine pushes its focused-element state into the
    // FrameLoop (via `onFocusChanged`, from the focus-change handler installed
    // at instance build); the per-frame `syncIme` (set up in `bind`) reads that
    // retained value and drives `showSoftInput` / `hideSoftInput`.

    override fun onCheckIsTextEditor(): Boolean = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        val inst = instance ?: return null
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT
        // Avoid the fullscreen extract pane (phones, landscape) — the real
        // editor is our canvas; the extract UI would diverge from it.
        outAttrs.imeOptions =
            EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        return object : BaseInputConnection(this, false) {
            override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
                val s = text.toString()
                if (s.isEmpty()) return super.commitText(s, newCursorPosition)
                if (s.length == 1 && s[0].code < 128 && !s[0].isISOControl()) {
                    // Single ASCII printable char → key-event path (matches
                    // direct keyboard typing; the engine inserts `key` on
                    // keydown). DOWN then UP.
                    inst.pushKey(s, "", 0, false, false, false, false)
                    inst.pushKey(s, "", 1, false, false, false, false)
                } else {
                    // Multi-char / non-ASCII → composition insert (paste,
                    // autocorrect, CJK direct-commit). CompositionStart then
                    // CompositionEnd{ text } makes the engine insert the whole
                    // string in one shot.
                    inst.pushIme(0, "")
                    inst.pushIme(2, s)
                }
                return true
            }

            override fun deleteSurroundingText(
                beforeChars: Int,
                afterChars: Int,
            ): Boolean {
                // Backspace → existing key path (engine deletes on "Backspace").
                inst.pushKey("Backspace", "Backspace", 0, false, false, false, false)
                inst.pushKey("Backspace", "Backspace", 1, false, false, false, false)
                return true
            }
        }
    }

    /**
     * Raise/lower the soft keyboard to match the engine's focused-element
     * state. The focused editable flag is pushed from native into the
     * [FrameLoop] ([onFocusChanged], fired by the engine's focus-change
     * handler each time focus / caret rect changes); this reads the retained
     * value and reconciles the IMM. State-gated so the IMM is only touched on
     * show↔hide transitions, not every frame. Suppressed until the user has
     * actually touched the surface ([userInteracted]) so a launch-time
     * programmatic focus doesn't pop the keyboard unprompted.
     */
    private fun syncIme() {
        val inst = instance ?: return
        val imm = context
            .getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        if (inst.focusedIsEditable() && userInteracted) {
            if (!hasFocus()) requestFocus()
            if (!imeActive) {
                imm.showSoftInput(this, 0)
                imeActive = true
            }
        } else if (imeActive) {
            imm.hideSoftInputFromWindow(windowToken, 0)
            imeActive = false
        }
    }
}
