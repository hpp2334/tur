// tur Compose integration library — an Android AAR that wraps the tur engine.
//
// The native `libtur_android.so` is built from `libs/tur-android` via
// `cargo ndk` and copied into `src/main/jniLibs/arm64-v8a/` by the
// `buildTurNative` task (run before `preBuild`). Embedders only need this AAR
// on their classpath — no manual NDK setup beyond having the Android NDK
// installed (and `ANDROID_NDK_HOME` set, or `ndk.dir` in local.properties).

import org.gradle.api.DefaultTask

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "ai.tur"
    compileSdk = 34

    defaultConfig {
        minSdk = 24

        // arm64-v8a only — the native .so is built for aarch64-linux-android.
        // This is the ABI real Android devices ship; the x86_64 emulator
        // build was a debug-only convenience that tripled the APK size.
        ndk {
            abiFilters += "arm64-v8a"
        }

        externalNativeBuild {
            // We don't use the AGP cmake/ndk-build path; the .so is produced by
            // the cargoNdkBuild task below. This block is just a marker.
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

    sourceSets {
        getByName("main") {
            // jniLibs is the standard AGP location for prebuilt .so files.
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.runtime:runtime")
}

// --- Native build: cargo ndk → jniLibs --------------------------------------

// Resolve the workspace root from THIS module's directory (not rootProject),
// since this AAR may be included via a relative path from a different gradle
// root (e.g. demo/compose). integrations/compose → repo root is two parents up.
val turWorkspaceRoot = projectDir.parentFile.parentFile
val turAndroidCrate = File(turWorkspaceRoot, "libs/tur-android")
val jniLibsArm64 = File(projectDir, "src/main/jniLibs/arm64-v8a")
// cargo builds into the workspace `target/` dir (shared across crates), so the
// artifact is here regardless of which crate we point `--manifest-path` at.
val cargoTargetDir = File(turWorkspaceRoot, "target/aarch64-linux-android")

val cargoProfile = project.findProperty("turCargoProfile") as String? ?: "release"
val cargoAbis = listOf("arm64-v8a" to "aarch64")
val cargoArtifacts = cargoAbis.map { (abi, arch) ->
    abi to File(turWorkspaceRoot, "target/$arch-linux-android/${cargoProfile}/libtur_android.so")
}

val buildTurNative by tasks.registering(Exec::class) {
    group = "tur"
    description = "Cross-compile libtur_android.so via cargo-ndk."
    inputs.dir(File(turAndroidCrate, "src"))
    inputs.file(File(turAndroidCrate, "Cargo.toml"))
    inputs.file(File(turWorkspaceRoot, "Cargo.toml"))
    cargoArtifacts.forEach { (_, file) -> outputs.file(file) }

    val profileArgs = if (cargoProfile == "release") listOf("--release") else emptyList()
    val targetArgs = cargoAbis.flatMap { (_, arch) -> listOf("-t", "$arch-linux-android") }
    commandLine(
        "cargo", "ndk",
        *targetArgs.toTypedArray(),
        "build",
        "--manifest-path", File(turAndroidCrate, "Cargo.toml").absolutePath,
        *profileArgs.toTypedArray(),
    )
    environment("ANDROID_NDK_HOME", System.getenv("ANDROID_NDK_HOME")
        ?: File(System.getenv("ANDROID_HOME") ?: "", "ndk/27.0.12077973").absolutePath)
}

val copyTurNative by tasks.registering(DefaultTask::class) {
    group = "tur"
    description = "Copy the built libtur_android.so files into jniLibs/."
    dependsOn(buildTurNative)
    cargoArtifacts.forEach { (_, artifact) -> inputs.file(artifact) }
    // Run the copies imperatively — each ABI's .so lands in its own subdir, so
    // a single Copy task with multiple `from`s would flag them as duplicates
    // (same filename `libtur_android.so`).
    doLast {
        cargoArtifacts.forEach { (abi, artifact) ->
            val dir = File(jniLibsArm64.parentFile, abi)
            dir.mkdirs()
            artifact.copyTo(File(dir, "libtur_android.so"), overwrite = true)
        }
    }
}

// Make sure the .so is present before the APK packages jniLibs. AGP 8.x names
// the jniLibs merge task `merge<Variant>JniLibFolders`.
android.libraryVariants.all {
    val variantCapitalized = name.replaceFirstChar { it.uppercase() }
    tasks.matching { it.name == "merge${variantCapitalized}JniLibFolders" }
        .configureEach { dependsOn(copyTurNative) }
}
