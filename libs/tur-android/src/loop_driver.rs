//! `LoopDriver` for the Android embedder, backed by Kotlin's `FrameLoop`.
//!
//! The engine owns the frame logic (clock advance is its own `StdClock`, no
//! manual forwarding); this driver just arms the next wake-up per the engine's
//! [`NextFrame`] verdict by calling into Kotlin's `FrameLoop`:
//! - [`NextFrame::Vsync`] → `FrameLoop.scheduleVsync()` (Android `Choreographer`)
//! - [`NextFrame::After(d)`] → `FrameLoop.scheduleDelayed(millis)`
//! - [`NextFrame::Idle`] → `FrameLoop.cancel()`
//!
//! The Kotlin side fires the engine's wake trampoline (`TurEngine.wake()` via
//! `nativePump`) when due. The wake trampoline and the Kotlin callback live in
//! a `Mutex` so the engine can set the wake after the driver is installed.

use std::rc::Rc;
use std::sync::Mutex;

use jni::objects::{GlobalRef, JObject};
use jni::JNIEnv;
use tur_engine::core::app::NextFrame;
use tur_engine::LoopDriver;

/// Handle to Kotlin's `org.tur.FrameLoop` object, stashed at create time so the
/// `LoopDriver` (which the engine calls from its own frame tick) can reach it.
#[derive(Clone)]
pub struct FrameLoopRef {
    /// Global ref to the Kotlin `FrameLoop` instance.
    pub kotlin_loop: GlobalRef,
}

impl FrameLoopRef {
    pub fn new(kotlin_loop: GlobalRef) -> Self {
        Self { kotlin_loop }
    }
}

/// The Android `LoopDriver`. Holds the Kotlin `FrameLoop` global ref and the
/// engine's wake trampoline (set once at `start`).
pub struct AndroidLoopDriver {
    pub frame_loop: FrameLoopRef,
    pub wake: Mutex<Option<Rc<dyn Fn()>>>,
}

impl AndroidLoopDriver {
    pub fn new(frame_loop: FrameLoopRef) -> Self {
        Self {
            frame_loop,
            wake: Mutex::new(None),
        }
    }

    /// Fire the engine wake trampoline. Called from JNI (`nativePump`) when
    /// the Kotlin `Choreographer` / `Handler` fires.
    pub fn fire(&self) {
        let wake = self.wake.lock().unwrap().clone();
        if let Some(wake) = wake {
            wake();
        }
    }
}

impl LoopDriver for AndroidLoopDriver {
    fn set_wake(&self, wake: Rc<dyn Fn()>) {
        *self.wake.lock().unwrap() = Some(wake);
    }

    fn request_next(&self, next: NextFrame) {
        // Attach to the JVM and call the Kotlin FrameLoop. The engine calls
        // this from its own thread (the frame loop), which the embedder has
        // already attached to the JVM.
        let vm = match crate::java_vm() {
            Some(vm) => vm,
            None => return,
        };
        let mut env = match vm.attach_current_thread() {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("loop driver: attach failed: {e}");
                return;
            }
        };
        let loop_obj = unsafe { JObject::from_raw(self.frame_loop.kotlin_loop.as_raw()) };
        match next {
            NextFrame::Idle => {
                call_void(&mut env, &loop_obj, "cancel", "()V", &[]);
            }
            NextFrame::Vsync => {
                call_void(&mut env, &loop_obj, "scheduleVsync", "()V", &[]);
            }
            NextFrame::After(delay) => {
                let ms = delay.as_millis().min(i64::MAX as u128) as i64;
                call_void(
                    &mut env,
                    &loop_obj,
                    "scheduleDelayed",
                    "(J)V",
                    &[jni::objects::JValue::Long(ms.max(1))],
                );
            }
        }
    }
}

fn call_void(env: &mut JNIEnv, obj: &JObject, name: &str, sig: &str, args: &[jni::objects::JValue]) {
    if let Err(e) = env.call_method(obj, name, sig, args) {
        tracing::warn!("loop driver: {name} failed: {e}");
    }
}
