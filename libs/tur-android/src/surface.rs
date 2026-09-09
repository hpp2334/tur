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

// SAFETY: the wrapped `ANativeWindow*` comes from `ANativeWindow_fromSurface`,
// which ACQUIRES a reference — the window object is process-global and stays
// alive while that ref is held, so moving the handle to another thread (the
// tur-host thread creates the wgpu surface from it) is sound. Deliberately
// NOT `Sync`: the handle models a single-owner borrow of the window, and
// concurrent use from multiple threads is not expressible here.
unsafe impl Send for AndroidWindowHandle {}

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

    /// The wrapped `ANativeWindow*`. Escape hatch for the attach/detach
    /// release protocol: the handle deliberately has no `Drop` (a naive
    /// drop-release would race the wgpu surface that borrows the window),
    /// so the owner releases explicitly via
    /// [`release_native_window`] — the attach op on failure, the detach
    /// op on success (after the renderer — and with it the wgpu surface —
    /// is dropped).
    pub fn as_ptr(&self) -> *mut c_void {
        self.a_native_window.as_ptr()
    }
}

impl HasWindowHandle for AndroidWindowHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: the `ANativeWindow*` is valid for the lifetime of this
        // `AndroidWindowHandle` (the embedder guarantees the Android `Surface`
        // is alive), satisfying `WindowHandle`'s validity invariant.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.raw_window_handle()) })
    }
}

impl HasDisplayHandle for AndroidWindowHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
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
        pub fn ANativeWindow_getWidth(window: *mut c_void) -> i32;
        pub fn ANativeWindow_getHeight(window: *mut c_void) -> i32;
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
/// The release half of the attach/detach pairing — see
/// [`AndroidWindowHandle::as_ptr`].
#[cfg(target_os = "android")]
pub unsafe fn release_native_window(window: *mut c_void) {
    if !window.is_null() {
        unsafe { ffi::ANativeWindow_release(window) };
    }
}

#[cfg(not(target_os = "android"))]
pub unsafe fn release_native_window(_window: *mut c_void) {}

/// Query an `ANativeWindow`'s buffer size in PHYSICAL pixels. The ground
/// truth for the embedder unit contract: the engine expects `width/height`
/// in logical units and `dpr` to scale them to this buffer size. Returns
/// `None` on non-Android targets or a null window.
pub fn native_window_size(window: *mut c_void) -> Option<(u32, u32)> {
    if window.is_null() {
        return None;
    }
    #[cfg(target_os = "android")]
    unsafe {
        let w = ffi::ANativeWindow_getWidth(window);
        let h = ffi::ANativeWindow_getHeight(window);
        if w > 0 && h > 0 {
            Some((w as u32, h as u32))
        } else {
            None
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Cross-check the embedder-declared logical size × dpr against the
/// window's real buffer size (physical px), logging a loud error on
/// mismatch. The engine's layout is logical-unit only and scales the
/// painted scene by `dpr` at the render surface — it has no internal way
/// to detect a caller passing physical px as logical (or a density that
/// doesn't match the window's real px/dp under display zoom /
/// compatibility scaling). Such a mismatch silently scales every
/// fixed-size prop by the dpr factor; the classic symptom is text
/// ellipsizing at ~1/dpr of its width budget while glyphs paint
/// normally. Log-not-panic: a wrong-size surface still renders
/// (degraded), matching the renderer's device-quirk policy.
///
/// Android-only: on other targets `native_window_size` is always `None`
/// and the check is meaningless (the attach op fails earlier on the null
/// window stub anyway).
#[cfg(target_os = "android")]
pub fn check_logical_dpr_against_window(window: *mut c_void, width: i32, height: i32, dpr: f64) {
    let Some((buf_w, buf_h)) = native_window_size(window) else {
        return;
    };
    let expected_w = width as f64 * dpr;
    let expected_h = height as f64 * dpr;
    // Tolerance: ~2 logical px of rounding between the reported size and
    // the native buffer.
    let tol = 2.0 * dpr.max(1.0);
    if (buf_w as f64 - expected_w).abs() > tol || (buf_h as f64 - expected_h).abs() > tol {
        log::error!(
            "tur attach: logical size {}x{} @{dpr}x (= {expected_w:.0}x{expected_h:.0} \
             physical) does not match the ANativeWindow buffer {buf_w}x{buf_h}. The engine \
             lays out in LOGICAL units and scales the scene by dpr at paint — passing \
             physical px as logical (or a mismatched density) silently scales every \
             fixed-size prop by the dpr factor (symptom: text ellipsizes at ~1/dpr of \
             its width budget with normal-size glyphs). Fix the caller's px↔dp conversion \
             — e.g. TurView divides holder.surfaceFrame (physical px) by \
             Resources.displayMetrics.density.",
            width.max(1),
            height.max(1),
        );
    }
}
