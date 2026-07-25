//! wgpu `Surface` creation from an Android `Surface` (via its underlying
//! `ANativeWindow*`). On non-Android targets this module is empty (the crate
//! compiles as a stub so the workspace builds on desktop).
//!
//! The flow: the embedder hands us a JNI ref to an Android `Surface`; we call
//! `ANativeWindow_fromSurface` to get the raw `ANativeWindow*`, wrap it in a
//! `raw-window-handle` 0.6 `AndroidNdkWindowHandle`, and create a wgpu surface
//! via `instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { .. })`
//! — exactly the pattern the native vello test harness uses.

use std::ffi::c_void;
use std::ptr::NonNull;

use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

/// The handle wgpu receives. Owns no resources itself — the lifetime of the
/// underlying `ANativeWindow*` is the embedder's responsibility (the Android
/// `SurfaceView` keeps it valid until `surfaceDestroyed`).
pub struct AndroidWindowHandle {
    a_native_window: NonNull<c_void>,
}

impl AndroidWindowHandle {
    /// Wrap a raw `ANativeWindow*` (non-null) obtained via
    /// `ANativeWindow_fromSurface`. The handle is borrowed for the duration of
    /// the `AndroidWindowHandle`; the embedder must NOT release the window
    /// before this is dropped.
    pub unsafe fn new(ptr: *mut c_void) -> Self {
        Self {
            a_native_window: NonNull::new(ptr).expect("ANativeWindow was null"),
        }
    }

    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(self.a_native_window))
    }

    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::new())
    }
}

impl HasWindowHandle for AndroidWindowHandle {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: the `ANativeWindow*` is valid for the lifetime of this
        // `AndroidWindowHandle` (the embedder guarantees the Android `Surface`
        // is alive), satisfying `WindowHandle`'s validity invariant.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.raw_window_handle()) })
    }
}

impl HasDisplayHandle for AndroidWindowHandle {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: the Android display handle carries no pointers (it's a
        // zero-sized marker), so borrowing it raw is always sound.
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(self.raw_display_handle()) })
    }
}

#[cfg(target_os = "android")]
mod ffi {
    use std::ffi::c_void;

    unsafe extern "C" {
        // libandroid.so
        pub fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
        #[allow(dead_code)]
        pub fn ANativeWindow_release(window: *mut c_void);
    }
}

/// Obtain the raw `ANativeWindow*` behind an Android `Surface` jobject. The
/// caller must `ANativeWindow_release` it when done (or hand it to an
/// `AndroidWindowHandle` for a borrowed view without acquiring ownership).
///
/// On non-Android targets this is a no-op stub.
#[cfg(target_os = "android")]
pub unsafe fn native_window_from_surface(env: *mut c_void, surface: *mut c_void) -> *mut c_void {
    unsafe { ffi::ANativeWindow_fromSurface(env, surface) }
}

#[cfg(not(target_os = "android"))]
pub unsafe fn native_window_from_surface(_env: *mut c_void, _surface: *mut c_void) -> *mut c_void {
    std::ptr::null_mut()
}

/// Release a window previously acquired via [`native_window_from_surface`].
#[cfg(target_os = "android")]
#[allow(dead_code)]
pub unsafe fn release_native_window(window: *mut c_void) {
    if !window.is_null() {
        unsafe { ffi::ANativeWindow_release(window) };
    }
}

#[cfg(not(target_os = "android"))]
pub unsafe fn release_native_window(_window: *mut c_void) {}
