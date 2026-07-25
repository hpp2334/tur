//! tur playground demo — the Android `.so` for the demo app.
//!
//! This is the **embedder's** crate: it links [`tur_android`] (the reusable
//! engine glue, as an rlib) and layers the demo's plugin set on top. The
//! standard engine-operation JNI symbols (`Java_org_tur_TurNative_*` — pump,
//! input, IME, …) come from [`tur_android::standard_jni_exports!`]; the
//! **creation** symbol (`Java_org_tur_demo_DemoNative_createEngine`) is
//! hand-written here because the plugin set varies per embedder.
//!
//! Copy this crate as the template for your own app's native lib: change the
//! crate name, swap the plugin list in `createEngine`, and ship the resulting
//! `.so`.

// Everything is Android-only — on a host `cargo check --workspace` this is an
// empty (but compiling) cdylib.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

// Standard engine-op JNI trampolines (loadModule / pump / resize / pushPointer
// / pushKey / focusedIsEditable / pushIme / destroy). Resolved by Kotlin's
// `org.tur.TurNative` bridge.
#[cfg(target_os = "android")]
tur_android::standard_jni_exports!();

/// `DemoNative.createEngine(env, context, surface, w, h, dpr, frameLoop): long`
///
/// Builds the engine with the **demo's** plugins (Std + Animation + Clipboard +
/// Net + DemoHelper) over the Android `Surface`. Returns an opaque handle
/// Kotlin holds as a `long` and passes back to the standard ops above. The app
/// loads this `.so` and calls here on surface-ready.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_tur_demo_DemoNative_createEngine(
    mut env: tur_android::JNIEnv,
    _class: tur_android::JClass,
    context: tur_android::JObject,
    surface: tur_android::JObject,
    width: tur_android::jint,
    height: tur_android::jint,
    dpr: tur_android::jdouble,
    frame_loop: tur_android::JObject,
) -> tur_android::jlong {
    use tur_animation::TurAnimationPlugin;
    use tur_demo_plugin::TurDemoPlugin;
    use tur_engine::{TurClipboardPlugin, TurStdPlugin};
    use tur_net_native::TurNetPlugin;

    tur_android::ops::create_with_plugins(
        &mut env,
        context,
        surface,
        width,
        height,
        dpr,
        frame_loop,
        |builder| {
            builder
                .plugin(TurStdPlugin)
                .plugin(TurAnimationPlugin)
                .plugin(TurClipboardPlugin)
                .plugin(TurNetPlugin)
                .plugin(TurDemoPlugin)
        },
    )
}
