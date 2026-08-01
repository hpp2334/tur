//! Android clipboard backend for tur, backed by the Android
//! `android.content.ClipboardManager` (reached via JNI).
//!
//! Re-exports the clipboard plugin surface from [`tur_engine`] so Android
//! embedders only need this one crate. The backend ([`AndroidClipboard`]) is
//! registered via
//! `TurRuntimeBuilder::capability(Clipboard::new(AndroidClipboard::new(context)))`,
//! where `context` is a JNI global ref to the Android app `Context`.
//!
//! The process `JavaVM` must be registered once via [`set_java_vm`] from the
//! embedder's JNI entry point (the first `Java_*` invocation); the backend
//! attaches the current thread on each call to reach `ClipboardManager`.
//!
//! On non-Android targets this crate compiles as a near-empty stub (only the
//! re-exports surface).

pub use tur_engine::{Clipboard, ClipboardBackend, TurClipboardPlugin};

#[cfg(target_os = "android")]
mod backend;

#[cfg(target_os = "android")]
pub use backend::{AndroidClipboard, set_java_vm};
