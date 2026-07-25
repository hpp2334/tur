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
 * A Compose surface that runs a tur engine instance and renders the given JS.
 *
 * The single-call integration point: drop this composable into any Compose UI,
 * pass a JS bundle string (an ES module importing from `tur:std` / `tur:animation`
 * / etc.), and tur renders into the surface. Pointer (touch), resize, and the
 * frame loop are wired automatically; basic key dispatch is wired when the
 * surface has focus.
 *
 * Example:
 * ```
 * val js = remember { context.assets.open("playground.js").bufferedReader().use { it.readText() } }
 * TurView(js = js, modifier = Modifier.fillMaxSize())
 * ```
 *
 * @param js an ES module source (the bundle produced by rspack from
 *   `tur-demo-impl` or a `tur-test-cases` case). Imports of `tur:*` /
 *   `tur-ext/demo-helper` are resolved by the engine's module loader.
 * @param dpr force a DPR (defaults to the window's `Resources.displayMetrics.density`).
 */
@Composable
fun TurView(
    js: String,
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
        surfaceView.bind(js, context, resolvedDpr)
        onDispose { surfaceView.unbind() }
    }
}

/**
 * `SurfaceView` subclass that owns the [TurEngine] lifecycle + input dispatch.
 *
 * The engine is created lazily via [bind] (called once the surface is ready —
 * see [TurView]'s `DisposableEffect`). All methods must be called on the main
 * looper (where `SurfaceHolder.Callback` and input dispatch arrive).
 */
private class TurSurfaceView(context: android.content.Context) : SurfaceView(context) {
    private var engine: TurEngine? = null
    private var pendingJs: String? = null
    private var dprValue: Double = 0.0
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

    /** Stash the JS + dpr and register the surface callback; create the engine
     *  when the surface is ready. */
    fun bind(js: String, context: android.content.Context, dpr: Double) {
        pendingJs = js
        dprValue = dpr
        isFocusable = true
        isFocusableInTouchMode = true
        requestFocus()
        holder.addCallback(surfaceCallback)
        setOnTouchListener { _, event ->
            userInteracted = true
            val eng = engine ?: return@setOnTouchListener false
            // `MotionEvent.getX/Y` are in physical px (Android's view coord
            // space); the engine hit-tests in logical px, so divide by dpr to
            // land taps in the same space as the layout.
            val dpr = dprValue.coerceAtLeast(1.0)
            eng.pushPointer(
                event.actionMasked,
                event.x.toDouble() / dpr,
                event.y.toDouble() / dpr,
                event.eventTime,
            )
            true
        }
    }

    /** Tear down: remove callbacks + destroy the engine. */
    fun unbind() {
        holder.removeCallback(surfaceCallback)
        setOnTouchListener(null)
        engine?.setAfterPump(null)
        engine?.close()
        engine = null
        imeActive = false
    }

    private val surfaceCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (engine != null) return
            val js = pendingJs ?: return
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
            engine = try {
                TurEngine.create(context, holder.surface, w, h, dprValue).also {
                    it.loadModule(js)
                    // After each frame, sync the soft keyboard with the engine's
                    // focused-element state (polls `focusedIsEditable`).
                    it.setAfterPump { syncIme() }
                }
            } catch (e: Throwable) {
                android.util.Log.e("TurView", "engine create failed", e)
                null
            }
        }

        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            // `width`/`height` here are physical px (same unit as
            // `surfaceFrame`); convert to logical px before pushing to the
            // engine. See `surfaceCreated` for the unit rationale.
            val dpr = dprValue.coerceAtLeast(1.0)
            engine?.resize(
                (width / dpr).toInt().coerceAtLeast(1),
                (height / dpr).toInt().coerceAtLeast(1),
                dprValue,
            )
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {
            engine?.close()
            engine = null
        }
    }

    override fun onKeyDown(keyCode: Int, event: android.view.KeyEvent): Boolean {
        val eng = engine ?: return super.onKeyDown(keyCode, event)
        val mapped = InputMapper.map(keyCode) ?: return super.onKeyDown(keyCode, event)
        eng.pushKey(
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
        val eng = engine ?: return super.onKeyUp(keyCode, event)
        val mapped = InputMapper.map(keyCode) ?: return super.onKeyUp(keyCode, event)
        eng.pushKey(
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
    // engine events; the per-frame `syncIme` poll (set up in `bind`) drives
    // `showSoftInput` / `hideSoftInput` from the engine's focused-element state.

    override fun onCheckIsTextEditor(): Boolean = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        val eng = engine ?: return null
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
                    eng.pushKey(s, "", 0, false, false, false, false)
                    eng.pushKey(s, "", 1, false, false, false, false)
                } else {
                    // Multi-char / non-ASCII → composition insert (paste,
                    // autocorrect, CJK direct-commit). CompositionStart then
                    // CompositionEnd{ text } makes the engine insert the whole
                    // string in one shot.
                    eng.pushIme(0, "")
                    eng.pushIme(2, s)
                }
                return true
            }

            override fun deleteSurroundingText(
                beforeChars: Int,
                afterChars: Int,
            ): Boolean {
                // Backspace → existing key path (engine deletes on "Backspace").
                eng.pushKey("Backspace", "Backspace", 0, false, false, false, false)
                eng.pushKey("Backspace", "Backspace", 1, false, false, false, false)
                return true
            }
        }
    }

    /**
     * Poll the engine's focused-element state (after each pump) and raise/lower
     * the soft keyboard accordingly. State-gated so the IMM is only touched on
     * show↔hide transitions, not every frame. Suppressed until the user has
     * actually touched the surface ([userInteracted]) so a launch-time
     * programmatic focus doesn't pop the keyboard unprompted.
     */
    private fun syncIme() {
        val eng = engine ?: return
        val imm = context
            .getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        if (eng.focusedIsEditable() && userInteracted) {
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
