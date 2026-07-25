package org.tur.demo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import org.tur.TurView
import java.io.IOException

/**
 * tur playground on Android.
 *
 * Loads the prebuilt `playground.js` asset (the full tur-demo-impl bundle —
 * sidebar + editor + viewer with all ~80 cases) into a single [TurView], which
 * builds the native engine over the activity's surface and renders into it.
 *
 * If the asset is missing or unreadable (e.g. the gradle `copyPlaygroundJs`
 * task didn't run), an error message is shown instead of crashing.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            Surface(modifier = Modifier.fillMaxSize(), color = Color.White) {
                val js = remember { loadAsset("playground.js") }
                js?.let {
                    TurView(js = it, modifier = Modifier.fillMaxSize())
                } ?: run {
                    androidx.compose.material3.Text(
                        "playground.js asset not found.\n" +
                            "Run ./gradlew copyPlaygroundJs first.",
                        color = Color.Red,
                    )
                }
            }
        }
    }

    private fun loadAsset(name: String): String? = try {
        assets.open(name).bufferedReader().use { it.readText() }
    } catch (e: IOException) {
        android.util.Log.e("MainActivity", "failed to read asset $name", e)
        null
    }
}
