//! tur playground demo — the Android `.so` for the demo app.
//!
//! This is the **embedder's** crate: it links [`tur_android`] (the reusable
//! engine glue, as an rlib) and layers the demo's plugin set on top. The
//! standard engine-operation JNI symbols (`Java_org_tur_TurNative_*` —
//! createInstance, pump, input, IME, …) come from
//! [`tur_android::standard_jni_exports!`]; the **runtime-creation** symbol
//! (`Java_org_tur_demo_DemoNative_createRuntime`) is hand-written here because
//! the plugin set varies per embedder.
//!
//! Copy this crate as the template for your own app's native lib: change the
//! crate name, swap the plugin list in `createRuntime`, and ship the resulting
//! `.so`.

// Everything is Android-only — on a host `cargo check --workspace` this is an
// empty (but compiling) cdylib.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

// Standard engine-op JNI trampolines (createInstance / createHeadlessInstance
// / loadModule / pump / resize / pushPointer / pushKey / pushIme / destroy
// / destroyRuntime). Resolved by Kotlin's `org.tur.TurNative` bridge.
#[cfg(target_os = "android")]
tur_android::standard_jni_exports!();

/// `DemoNative.createRuntime(env, context): long`
///
/// Builds the shared runtime with the **demo's** plugins (Std + Animation +
/// Clipboard + Net + DemoHelper). Returns an opaque runtime handle Kotlin
/// holds as a `long` and passes to `TurNative.createInstance` /
/// `createHeadlessInstance`. No surface — instances are attached separately.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_tur_demo_DemoNative_createRuntime(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
) -> tur_android::jlong {
    use tur_animation::TurAnimationPlugin;
    use tur_engine::{TurClipboardPlugin, TurStdPlugin};
    use tur_net_native::TurNetPlugin;
    use tur_playground_plugin::TurPlaygroundPlugin;

    tur_android::ops::create_runtime(&mut env, context, |builder| {
        builder
            .plugin(TurStdPlugin)
            .plugin(TurAnimationPlugin)
            .plugin(TurClipboardPlugin)
            .plugin(TurNetPlugin)
            .plugin(TurPlaygroundPlugin)
    })
}

/// Minimal NDK asset FFI (libandroid.so). Kept local to the demo — reading an
/// APK asset natively is exactly the kind of embedder-specific glue that
/// belongs in the app's `.so`, not the reusable engine crate.
#[cfg(target_os = "android")]
mod asset_source {
    use std::ffi::{CString, c_char, c_void};

    #[repr(C)]
    pub struct AAssetManager {
        _unused: [u8; 0],
    }

    #[repr(C)]
    pub struct AAsset {
        _unused: [u8; 0],
    }

    unsafe extern "C" {
        fn AAssetManager_fromJava(
            env: *mut c_void,
            asset_manager: *mut c_void,
        ) -> *mut AAssetManager;
        fn AAssetManager_open(
            mgr: *mut AAssetManager,
            filename: *const c_char,
            mode: i32,
        ) -> *mut AAsset;
        fn AAsset_getLength(asset: *mut AAsset) -> u64;
        fn AAsset_read(asset: *mut AAsset, buf: *mut c_void, count: usize) -> i32;
        fn AAsset_close(asset: *mut AAsset);
    }

    /// Read the APK asset at `path` fully into a `String`.
    /// `asset_manager` is a JNI ref to an `android.content.res.AssetManager`
    /// (Kotlin: `context.assets`). The bytes never cross the JNI boundary —
    /// only the final, registered source handle does.
    pub fn read_asset(
        env: &mut tur_android::JNIEnv,
        asset_manager: tur_android::JObject,
        path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mgr = unsafe {
            AAssetManager_fromJava(
                env.get_raw() as *mut c_void,
                asset_manager.as_raw() as *mut c_void,
            )
        };
        if mgr.is_null() {
            return Err("AAssetManager_fromJava returned null".into());
        }
        let c_path = CString::new(path)?;
        // 3 == AASSET_MODE_BUFFER: read the whole asset up front.
        let asset = unsafe { AAssetManager_open(mgr, c_path.as_ptr(), 3) };
        if asset.is_null() {
            return Err(format!("asset not found: {path}").into());
        }
        let len = unsafe { AAsset_getLength(asset) } as usize;
        let mut buf = vec![0u8; len];
        // `AAsset_read` may return short reads — loop until full or EOF.
        let mut filled = 0usize;
        while filled < len {
            let n = unsafe {
                AAsset_read(
                    asset,
                    buf[filled..].as_mut_ptr() as *mut c_void,
                    len - filled,
                )
            };
            if n <= 0 {
                break;
            }
            filled += n as usize;
        }
        unsafe { AAsset_close(asset) };
        buf.truncate(filled);
        Ok(String::from_utf8(buf)?)
    }
}

/// `DemoNative.createAssetModuleSource(env, runtimeHandle, path, assetManager): long`
///
/// Reads the APK asset at `path` **entirely on the Rust side** (via the NDK
/// `AAssetManager` — no JNI string/byte traffic), registers it as a module
/// source on the runtime's registry, and returns the source handle. `TurView`
/// later loads it into an instance by handle, so the JS bundle never crosses
/// the Kotlin↔Rust boundary as a string. Returns `0` on failure (a
/// `RuntimeException` is also thrown).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_tur_demo_DemoNative_createAssetModuleSource(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    runtime_handle: tur_android::jlong,
    path: tur_android::JString,
    asset_manager: tur_android::JObject,
) -> tur_android::jlong {
    let result: Result<tur_android::jlong, Box<dyn std::error::Error>> = (|| {
        let path: String = env.get_string(&path)?.into();
        let source = asset_source::read_asset(&mut env, asset_manager, &path)?;
        log::info!(
            "createAssetModuleSource: {} read natively ({} bytes)",
            path,
            source.len()
        );
        let handle = tur_android::ops::with_runtime(runtime_handle, |rt| {
            rt.module_sources.register(source) as tur_android::jlong
        })
        .ok_or("invalid runtime handle")?;
        Ok(handle)
    })();
    match result {
        Ok(v) => v,
        Err(e) => {
            log::error!("createAssetModuleSource failed: {e}");
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("createAssetModuleSource: {e}"),
            );
            0
        }
    }
}
