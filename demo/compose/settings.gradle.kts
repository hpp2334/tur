// Root settings for the tur Android demo app.
//
// `demo/compose` is both the gradle root project and the application module
// (kept flat — one module, one APK). The tur Compose integration library is
// included via a relative path back into the repo at `integrations/compose`,
// so a single `./gradlew assembleArm64Debug` from here builds the AAR (and its
// native .so), then the APK.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
    // Plugin versions resolved centrally so the build files can use
    // `id("…")` without a version.
    val agpVersion = "8.7.3"
    val kotlinVersion = "1.9.25"
    plugins {
        id("com.android.application") version agpVersion
        id("com.android.library") version agpVersion
        id("org.jetbrains.kotlin.android") version kotlinVersion
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "tur-android-demo"

// The tur Compose integration library, reached via a relative path.
include(":tur-compose")
project(":tur-compose").projectDir = file("../../integrations/compose")
