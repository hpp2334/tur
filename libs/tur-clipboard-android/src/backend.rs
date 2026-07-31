//! [`ClipboardBackend`] impl backed by Android's `ClipboardManager` via JNI.
//!
//! The backend owns a `JavaVM` + a `GlobalRef` to the app `Context`, both
//! captured from the embedder's JNI entry point. Each call attaches the
//! current thread to the JVM (no-op if already attached) and reaches
//! `ClipboardManager` via `Context.getSystemService("clipboard")`. This avoids
//! the backend trait's context-free `&self` needing a stashed `JNIEnv` (which
//! is thread/borrow-scoped and can't be held across the engine's frame loop).

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use tur_engine::ClipboardBackend;

static VM: OnceLock<&'static JavaVM> = OnceLock::new();

/// Stash the process `JavaVM` (as a `'static` borrow — tur-android keeps one
/// boxed forever). Called once by tur-android's JNI entry point (the first
/// `Java_*` invocation, which receives a `JNIEnv` from which `get_java_vm()`
/// derives the `JavaVM`).
pub fn set_java_vm(vm: &'static JavaVM) {
    let _ = VM.set(vm);
}

/// Android clipboard backend. Constructed with a JNI global ref to the app
/// `Context`; reaches `ClipboardManager` lazily on each call.
pub struct AndroidClipboard {
    /// Global ref to `android.content.Context`.
    context: GlobalRef,
}

impl AndroidClipboard {
    pub fn new(context: GlobalRef) -> Self {
        Self { context }
    }
}

impl ClipboardBackend for AndroidClipboard {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> {
        let text = read_or_empty(&self.context);
        Box::pin(std::future::ready(text))
    }

    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = ()>>> {
        write_or_drop(&self.context, &text);
        Box::pin(std::future::ready(()))
    }
}

fn read_or_empty(context: &GlobalRef) -> String {
    let Some(vm) = VM.get() else {
        tracing::warn!("android clipboard: JavaVM not set");
        return String::new();
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("android clipboard: attach failed: {e}");
            return String::new();
        }
    };
    let context = unsafe { JObject::from_raw(context.as_raw()) };
    match read_text_sync(&mut env, &context) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("android clipboard read failed: {e}");
            String::new()
        }
    }
}

fn write_or_drop(context: &GlobalRef, text: &str) {
    let Some(vm) = VM.get() else {
        tracing::warn!("android clipboard: JavaVM not set");
        return;
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("android clipboard: attach failed: {e}");
            return;
        }
    };
    let context = unsafe { JObject::from_raw(context.as_raw()) };
    if let Err(e) = write_text_sync(&mut env, &context, text) {
        tracing::warn!("android clipboard write failed: {e}");
    }
}

/// Map a JNI `Result<T, E>` into `Result<T, String>` for the helper fns below.
trait ToStrErr<T> {
    fn str_err(self) -> Result<T, String>;
}
impl<T, E: std::fmt::Display> ToStrErr<T> for Result<T, E> {
    fn str_err(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

/// `clipboardManager.getPrimaryClip().getItemAt(0).coerceToText(context)`.
fn read_text_sync(env: &mut JNIEnv, context: &JObject) -> Result<String, String> {
    // Resolve the ClipboardManager via raw handles so each lookup releases its
    // mutable borrow of `env` before the next call (jni 0.21 ties every
    // `JObject<'a>` to the env's `'a`, which would otherwise forbid chaining).
    let clipboard_raw = {
        let service = env.new_string("clipboard").str_err()?;
        let v = env
            .call_method(
                context,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&service)],
            )
            .str_err()?;
        v.l()
            .map_err(|e| format!("getSystemService cast: {e}"))?
            .as_raw()
    };
    let clipboard = unsafe { JObject::from_raw(clipboard_raw) };

    // getPrimaryClip() returns android.content.ClipData (or null).
    let clip_raw = {
        let v = env
            .call_method(
                &clipboard,
                "getPrimaryClip",
                "()Landroid/content/ClipData;",
                &[],
            )
            .str_err()?;
        v.l()
            .map_err(|e| format!("getPrimaryClip cast: {e}"))?
            .as_raw()
    };
    if clip_raw.is_null() {
        return Ok(String::new());
    }
    let clip = unsafe { JObject::from_raw(clip_raw) };

    // clip.getItemAt(0) -> android.content.ClipData$Item
    let item_raw = {
        let v = env
            .call_method(
                &clip,
                "getItemAt",
                "(I)Landroid/content/ClipData$Item;",
                &[JValue::Int(0)],
            )
            .str_err()?;
        v.l().map_err(|e| format!("getItemAt cast: {e}"))?.as_raw()
    };
    if item_raw.is_null() {
        return Ok(String::new());
    }
    let item = unsafe { JObject::from_raw(item_raw) };

    // item.coerceToText(context) -> CharSequence (CharSequence is an interface)
    let cs_raw = {
        let v = env
            .call_method(
                &item,
                "coerceToText",
                "(Landroid/content/Context;)Ljava/lang/CharSequence;",
                &[JValue::Object(context)],
            )
            .str_err()?;
        v.l()
            .map_err(|e| format!("coerceToText cast: {e}"))?
            .as_raw()
    };
    if cs_raw.is_null() {
        return Ok(String::new());
    }
    let cs = unsafe { JObject::from_raw(cs_raw) };

    // CharSequence -> String via Object.toString()
    let s_raw = {
        let v = env
            .call_method(&cs, "toString", "()Ljava/lang/String;", &[])
            .str_err()?;
        v.l().map_err(|e| format!("toString cast: {e}"))?.as_raw()
    };
    let jstr: JString = unsafe { JObject::from_raw(s_raw) }.into();
    // Copy out the Java string into an owned Rust `String` (the `JavaStr`
    // borrows `env`, so we materialize before returning).
    let java_str = env.get_string(&jstr).str_err()?;
    let owned = java_str
        .to_str()
        .map_err(|e| format!("to_str: {e}"))
        .map(|s| s.to_owned())?;
    Ok(owned)
}

/// `ClipboardManager.setPrimaryClip(ClipData.newPlainText("tur", text))`.
fn write_text_sync(env: &mut JNIEnv, context: &JObject, text: &str) -> Result<(), String> {
    // Resolve the ClipboardManager first, then build + apply the ClipData in a
    // second step (the service handle borrows `env`, so we drop it before the
    // next mutable call by going through a raw handle).
    let clipboard_raw = {
        let service = env.new_string("clipboard").str_err()?;
        let v = env
            .call_method(
                context,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&service)],
            )
            .str_err()?;
        let obj = v.l().map_err(|e| format!("getSystemService cast: {e}"))?;
        obj.as_raw()
    };
    let clipboard = unsafe { JObject::from_raw(clipboard_raw) };
    // ClipData.newPlainText(CharSequence label, CharSequence text)
    let clip_raw = {
        let label = env.new_string("tur").str_err()?;
        let text_j = env.new_string(text).str_err()?;
        let clip_data_class = env.find_class("android/content/ClipData").str_err()?;
        let v = env
            .call_static_method(
                JClass::from(clip_data_class),
                "newPlainText",
                "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
                &[JValue::Object(&label), JValue::Object(&text_j)],
            )
            .str_err()?;
        let obj = v.l().map_err(|e| format!("newPlainText cast: {e}"))?;
        obj.as_raw()
    };
    let clip = unsafe { JObject::from_raw(clip_raw) };
    env.call_method(
        &clipboard,
        "setPrimaryClip",
        "(Landroid/content/ClipData;)V",
        &[JValue::Object(&clip)],
    )
    .str_err()?;
    Ok(())
}
