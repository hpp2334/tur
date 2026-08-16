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
 * [DemoNative]) with the demo plugin set, then reads the prebuilt
 * `playground.js` asset **natively on the Rust side**
 * ([DemoNative.createAssetModuleSource] — NDK AAssetManager) and hands the
 * resulting module-source handle to [TurView]. The bundle never crosses the
 * Kotlin↔Rust boundary as a string; it loads into the instance by handle.
 *
 * If the asset is missing or unreadable (e.g. the gradle `copyPlaygroundJs`
 * task didn't run), an error message is shown instead of crashing.
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
                // Read the playground bundle natively and register it as a
                // module source (released automatically on dispose). A
                // failed read throws from native — fall back to the error UI.
                val sourceHandle = rememberTurModuleSource(runtime) { rt ->
                    runCatching {
                        DemoNative.createAssetModuleSource(rt.handle, "playground.js", assets)
                    }.getOrDefault(0L)
                }
                if (sourceHandle != 0L) {
                    // TurView spawns an isolated instance from the runtime
                    // per surface and loads the registered source by handle.
                    TurView(
                        runtime = runtime,
                        sourceHandle = sourceHandle,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Text(
                        "playground.js asset not found.\n" +
                            "Run ./gradlew copyPlaygroundJs first.",
                        color = Color.Red,
                    )
                }
            }
        }
    }
}
