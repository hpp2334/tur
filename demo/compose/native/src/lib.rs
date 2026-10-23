//! tur playground demo — the Android `.so` for the demo app.
//!
//! This is the **embedder's** crate: it links [`tur_android`] (the reusable
//! engine glue, as an rlib) and layers the demo's plugin set on top. The
//! standard engine-operation JNI symbols (`Java_org_tur_TurNative_*` —
//! createInstance, pump, input, IME, …) come from
//! [`tur_android::standard_jni_exports!`]; the **runtime-creation** symbol
//! (`Java_org_tur_demo_DemoNative_createRuntime`) and the playground's
//! registration symbol (`…_createPlaygroundModuleSource`) are hand-written
//! here because the plugin set and the module payload vary per embedder.
//!
//! The playground itself rides the rut rails: its sources are
//! `include_str!`-embedded below (the website's `playgroundSource()` root +
//! the declared `menu` child) and registered on the runtime's registries
//! Rust-side, so `TurView`'s `loadModule(handle)` boots the whole file tree
//! through the engine's multi-file rut rail. The APK ships no module asset.
//!
//! Copy this crate as the template for your own app's native lib: change the
//! crate name, swap the plugin list in `createRuntime`, embed your module
//! sources, and ship the resulting `.so`.

// Everything is Android-only — on a host `cargo check --workspace` this is an
// empty (but compiling) cdylib.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

// Standard engine-op JNI trampolines (createInstance / attachInstance
// / detachInstance / loadModule / pump / resize / pushPointer / pushKey
// / pushIme / destroy / destroySettled / destroyRuntime). Resolved by
// Kotlin's `org.tur.TurNative` bridge.
#[cfg(target_os = "android")]
tur_android::standard_jni_exports!();

// --- The embedded rut playground --------------------------------------------
//
// The same sources the website embeds: the handwritten root module, the
// GENERATED case registry (regenerate via the playground's gen-cases
// script), and the declared `menu` child. A `rut/` edit requires the direct
// `cargo ndk` rebuild — gradle's `buildDemoNative` inputs watch only this
// crate's `src/` (see the android-dev skill).

const PLAYGROUND_MOD_RUT: &str = include_str!("../../../../rut/playground/mod.rut");
const CASES_GEN_RUT: &str = include_str!("../../../../rut/playground/cases_gen.rut");
const PLAYGROUND_MENU_RUT: &str = include_str!("../../../../rut/playground/menu.rut");

/// The website's concatenation (`playgroundSource()`): the root module + the
/// generated case registry as one root source.
fn playground_root() -> String {
    format!("{PLAYGROUND_MOD_RUT}\n{CASES_GEN_RUT}")
}

/// `DemoNative.createRuntime(env, context): long`
///
/// Builds the shared runtime with the **demo's** plugins (Std + Animation +
/// Clipboard + Net + the playground's rut compile service + FilePicker over
/// a stub backend). Returns an opaque runtime handle Kotlin holds as a
/// `long` and passes to `TurNative.createInstance`. No surface — instances
/// are attached separately via `TurNative.attachInstance`.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_tur_demo_DemoNative_createRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
) -> tur_android::jlong {
    use tur_animation::TurAnimationPlugin;
    use tur_engine::{TurClipboardPlugin, TurStdPlugin};
    use tur_filepicker_capability::{FilePicker, TurFilePickerPlugin};
    use tur_net_native::TurNetPlugin;
    use tur_playground::TurRutPlaygroundPlugin;

    tur_android::ops::create_runtime(&mut env, context, |builder| {
        builder
            .plugin(TurStdPlugin)
            .plugin(TurAnimationPlugin)
            .plugin(TurClipboardPlugin)
            .plugin(TurNetPlugin)
            // The playground's rut compile service — the `pg_*` rows the
            // editor's Run/auto-run and the viewer's virtual-app source
            // registry ride.
            .plugin(TurRutPlaygroundPlugin)
            // Browser-shaped surface: the file-picker capability against a
            // stub backend (tur-android ships no platform picker), so the
            // pick rows exist and cases riding `save_file` (github-viewer)
            // compile — the actual save is a logged no-op. The plugin
            // hard-requires the capability, so the order matters.
            .capability(|_| Ok(FilePicker::new(filepicker_stub::NoFilePicker)))
            .plugin(TurFilePickerPlugin)
    })
}

/// The demo's file-picker stand-in (the JS-era demo did the same — an
/// in-bundle `tur:filepicker` stub): `pick` resolves empty (as if
/// cancelled) and `save` resolves immediately with a log line. The point is
/// the SURFACE, not the files.
#[cfg(target_os = "android")]
mod filepicker_stub {
    use std::future::Future;
    use std::pin::Pin;

    use tur_filepicker_capability::{FilePickerBackend, PickOptions, PickedFile, SaveOptions};

    pub struct NoFilePicker;

    impl FilePickerBackend for NoFilePicker {
        fn pick(&self, _opts: PickOptions) -> Pin<Box<dyn Future<Output = Vec<PickedFile>>>> {
            Box::pin(std::future::ready(Vec::new()))
        }

        fn save(
            &self,
            name: String,
            _bytes: Vec<u8>,
            _opts: SaveOptions,
        ) -> Pin<Box<dyn Future<Output = ()>>> {
            log::info!("saveFile({name}): no file-picker backend on Android — dropped");
            Box::pin(std::future::ready(()))
        }
    }
}

/// `DemoNative.createPlaygroundModuleSource(env, runtimeHandle): long`
///
/// Register the **embedded rut playground** entirely on the Rust side and
/// return the loadable root-source handle. The root source (`mod.rut` + the
/// generated case registry — the website's `playgroundSource()` shape) is
/// registered on the runtime's module-source registry, and the full file
/// set (root + the declared `menu` child) on its multi-file table under
/// that same handle — so `TurView`'s `loadModule(handle)` resolves the file
/// set and boots the whole tree through the engine's multi-file rut rail
/// (`TurApp::load_rut_module_files`). Nothing crosses the JNI boundary as
/// text, and the APK needs no staged module asset. Returns `0` on failure
/// (a `RuntimeException` is also thrown).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_tur_demo_DemoNative_createPlaygroundModuleSource(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
) -> tur_android::jlong {
    let result: Result<tur_android::jlong, Box<dyn std::error::Error>> = (|| {
        use std::collections::BTreeMap;
        use std::sync::Arc;

        use tur_engine::core::rut_runtime::RutModuleFiles;

        let root: Arc<str> = Arc::from(playground_root());
        let root_len = root.len();
        let mut mods = BTreeMap::new();
        mods.insert(Arc::from("menu"), Arc::from(PLAYGROUND_MENU_RUT));
        let files = RutModuleFiles {
            root: root.clone(),
            mods,
        };
        let handle = tur_android::ops::with_runtime(runtime_handle, move |rt| {
            let source_handle = rt.module_sources.register(root.clone());
            rt.module_files.register(source_handle, files);
            source_handle as tur_android::jlong
        })
        .ok_or("invalid runtime handle")?;
        log::info!(
            "createPlaygroundModuleSource: rut playground registered (root {root_len} bytes + the menu child) @ handle {handle}"
        );
        Ok(handle)
    })();
    match result {
        Ok(v) => v,
        Err(e) => {
            log::error!("createPlaygroundModuleSource failed: {e}");
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("createPlaygroundModuleSource: {e}"),
            );
            0
        }
    }
}
