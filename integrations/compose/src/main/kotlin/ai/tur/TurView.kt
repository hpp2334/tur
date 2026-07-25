package ai.tur

import android.view.SurfaceHolder
import android.view.SurfaceView
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
        engine?.close()
        engine = null
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
}
