package org.tur.demo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import org.tur.TurView
import org.tur.rememberTurModuleSource
import org.tur.rememberTurRuntime

/**
 * tur playground on Android.
 *
 * Builds the shared runtime once via the app's own `libtur_demo.so` (see
 * [DemoNative]) with the demo plugin set, then registers the **rut
 * playground** embedded in that `.so`
 * ([DemoNative.createPlaygroundModuleSource] — the root source + the
 * declared `menu` child, `include_str!`-embedded Rust-side) and hands the
 * resulting module-source handle to [TurView]. Nothing crosses the
 * Kotlin↔Rust boundary as text; `loadModule` resolves the handle as a
 * multi-file module and boots the whole tree through the engine's multi-file
 * rut rail. The APK ships no module asset.
 *
 * If the registration fails (e.g. the runtime handle is stale), an error
 * message is shown instead of crashing.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            Surface(modifier = Modifier.fillMaxSize(), color = Color.White) {
                // Build the shared runtime once (the demo's .so registers
                // the demo plugins and returns a runtime handle).
                val runtime = rememberTurRuntime { context ->
                    DemoNative.createRuntime(context)
                }
                // Register the embedded rut playground and load it into the
                // instance TurView spawns (released automatically on
                // dispose). A failed registration throws from native — fall
                // back to the error UI.
                val sourceHandle = rememberTurModuleSource(runtime) { rt ->
                    runCatching {
                        DemoNative.createPlaygroundModuleSource(rt.handle)
                    }.getOrDefault(0L)
                }
                if (sourceHandle != 0L) {
                    // TurView spawns an isolated instance from the runtime
                    // per surface and loads the registered playground (root
                    // + menu child) by handle.
                    TurView(
                        runtime = runtime,
                        sourceHandle = sourceHandle,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Text(
                        "rut playground registration failed.\n" +
                            "See logcat (tag: tur).",
                        color = Color.Red,
                    )
                }
            }
        }
    }
}
