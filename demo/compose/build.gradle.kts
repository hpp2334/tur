// tur Android demo app — runs the full tur playground (`playground.js`) on a
// single `TurView`. Build with `./gradlew assembleDebug` (arm64-v8a only;
// the native .so is aarch64-linux-android).

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "ai.tur.demo"
    compileSdk = 34

    defaultConfig {
        applicationId = "ai.tur.demo"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            // arm64-v8a only — real Android devices. The x86_64 emulator
            // build was removed; it tripled the APK size for a debug-only
            // smoke-test convenience.
            abiFilters += "arm64-v8a"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    composeOptions {
        kotlinCompilerExtensionVersion = "1.5.15"
    }
}

dependencies {
    implementation(project(":tur-compose"))

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.runtime:runtime")
}

// --- JS asset pipeline ------------------------------------------------------
//
// Build the playground bundle (`js/packages/tur-demo-impl` → `dist/impl.js`)
// and copy it into `src/main/assets/playground.js` before the APK packages
// assets. The bundle is the full tur playground UI (sidebar + editor + viewer)
// with all ~80 cases inlined as string sources by `scripts/gen-cases.cjs`.

val workspaceRoot = rootProject.projectDir.parentFile.parentFile // demo/compose → repo root
val playgroundPkg = File(workspaceRoot, "js/packages/tur-demo-impl")
val playgroundDist = File(playgroundPkg, "dist/impl.js")
val assetsDir = File(projectDir, "src/main/assets")
val playgroundAsset = File(assetsDir, "playground.js")

val buildPlaygroundJs by tasks.registering(Exec::class) {
    group = "tur"
    description = "Build the playground JS bundle (dist/impl.js) via pnpm."
    outputs.file(playgroundDist)
    inputs.file(File(playgroundPkg, "package.json"))
    inputs.dir(File(playgroundPkg, "src"))

    workingDir = File(workspaceRoot, "js")
    commandLine("pnpm", "--filter", "@tur/demo-impl", "build")
}

val copyPlaygroundJs by tasks.registering(Copy::class) {
    group = "tur"
    description = "Copy dist/impl.js into assets/playground.js."
    dependsOn(buildPlaygroundJs)
    from(playgroundDist)
    into(assetsDir)
    rename { "playground.js" }
}

// AGP 8.x: hook the asset copy into every variant's asset-merge task. The
// task name is `merge<Variant>Assets` (e.g. `mergeDebugAssets`).
android.applicationVariants.all {
    val variantCapitalized = name.replaceFirstChar { it.uppercase() }
    tasks.matching { it.name == "merge${variantCapitalized}Assets" }
        .configureEach { dependsOn(copyPlaygroundJs) }
    // Lint-vital tasks (`generate<Variant>LintVitalReportModel`,
    // `lintVitalAnalyze<Variant>`, …) also scan the source assets dir, so they
    // must run after the playground bundle is copied in — otherwise Gradle 8.x
    // rejects it as an undeclared implicit dependency. Match all lint-vital
    // tasks for the variant rather than enumerate names.
    tasks.matching {
        it.name.lowercase().contains("lintvital") &&
            it.name.contains(variantCapitalized)
    }.configureEach { dependsOn(copyPlaygroundJs) }
}

// Ensure the assets dir exists so the Copy task has a target even on a clean
// checkout.
assetsDir.mkdirs()
