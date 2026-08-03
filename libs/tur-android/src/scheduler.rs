//! Android scheduler driver (JNI-backed).
//!
//! Implements both [`MainScheduler`] and [`WorkerScheduler`] for Android.
//! Replaces the old `loop_driver.rs` (`LoopDriver`-based, now deleted).
//!
//! ## Vsync events
//!
//! `request_vsync` calls into Kotlin's `FrameLoop.scheduleVsync()` via JNI
//! (Choreographer-backed). When the Choreographer fires, Kotlin calls
//! `nativePump` which invokes [`AndroidSchedulerDriver::fire_vsync`],
//! pushing an event into every subscribed vsync channel.
//!
//! ## Frame loop ownership
//!
//! Each `AndroidInstance` owns its own Kotlin `FrameLoop` (Choreographer
//! callback), so the vsync driver is **per-instance**: `AndroidRuntime::build`
//! installs a base driver with no frame loop (its `request_vsync` no-ops —
//! the worker thread is all it needs), and each instance replaces the app's
//! main scheduler via `TurApp::set_main_scheduler` with a driver bound to its
//! own `FrameLoop`. Multiple subscribers per driver are supported (broadcast),
//! which is what the base driver's `vsync_events()` channel provides.
//!
//! ## Sleep
//!
//! `sleep(d)` returns a `Sleep(BoxFuture)` backed by `tokio::time::sleep`
//! via a oneshot channel. The driver holds a `tokio::runtime::Handle`
//! (shared with `HttpBackend`) for **timers only**.
//!
//! ## Spawn primitives
//!
//! `spawn_local` uses thread-local `futures::executor::LocalPool` per
//! thread (main + each worker). `block_on` uses the same pool.
//!
//! ## Worker spawn
//!
//! `spawn_worker` uses `std::thread::spawn` (dedicated OS thread,
//! guarantees main ≠ worker). The driver sets up the thread-local
//! LocalPool on the worker thread before invoking the factory.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;
use jni::objects::JObject;

use tur_engine::core::scheduler::{
    MainScheduler, Sleep, VsyncEvents, WorkerHandle, WorkerScheduler,
};

/// Handle to Kotlin's `org.tur.FrameLoop` object, stashed at create time so
/// the scheduler (which the engine calls from its own frame tick) can reach
/// it.
#[derive(Clone)]
pub struct FrameLoopRef {
    /// Global ref to the Kotlin `FrameLoop` instance.
    pub kotlin_loop: jni::objects::GlobalRef,
}

impl FrameLoopRef {
    pub fn new(kotlin_loop: jni::objects::GlobalRef) -> Self {
        Self { kotlin_loop }
    }
}
thread_local! {
    /// Per-thread `LocalPool` for `spawn_local` + `block_on`. Set by
    /// [`AndroidSchedulerDriver::new`] for the main thread and inside
    /// [`AndroidSchedulerDriver::spawn_worker`] for each worker thread.
    static CURRENT_POOL: RefCell<Option<LocalPool>> = const { RefCell::new(None) };
}

/// Helper: spawn a future on the current thread's LocalPool.
fn spawn_local_on_current_thread(fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
    CURRENT_POOL.with(|pool| {
        // We need a `&self` reference to call `spawner()`. RefCell::borrow
        // gives us that. The spawned future's lifecycle extends beyond
        // the borrow, but `LocalPool::spawner` returns a clonable handle
        // that doesn't borrow from the pool.
        let guard = pool.borrow();
        let pool_ref = guard
            .as_ref()
            .expect("spawn_local called with no LocalPool set on this thread");
        let _ = pool_ref.spawner().spawn_local(fut);
    });
}

/// Helper: drive a future to completion on the current thread's LocalPool.
fn block_on_on_current_thread(fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
    CURRENT_POOL.with(|pool| {
        let mut guard = pool.borrow_mut();
        let pool_ref = guard
            .as_mut()
            .expect("block_on called with no LocalPool set on this thread");
        pool_ref.run_until(fut);
    });
}

/// Construct a Sleep future backed by tokio::time::sleep bridged via oneshot.
fn tokio_sleep(runtime: Arc<tokio::runtime::Handle>, d: Duration) -> Sleep {
    let (tx, rx) = futures::channel::oneshot::channel();
    runtime.spawn(async move {
        tokio::time::sleep(d).await;
        let _ = tx.send(());
    });
    Sleep(Box::pin(async move {
        let _ = rx.await;
    }))
}

/// Android scheduler driver. Construct via [`AndroidSchedulerDriver::new`]
/// on the main thread (typically from `AndroidRuntime::build`).
pub struct AndroidSchedulerDriver {
    inner: Arc<AndroidInner>,
    /// Tokio runtime handle — shared with `HttpBackend` for reqwest.
    /// Used for **timers only** (no spawn_local / block_on).
    runtime: Arc<tokio::runtime::Handle>,
    /// JNI `FrameLoop` global ref. The driver's `request_vsync` calls
    /// `scheduleVsync()` on this object. `None` for the runtime base
    /// driver (no frame loop at runtime-build time; instances replace it).
    frame_loop: Option<FrameLoopRef>,
}

struct AndroidInner {
    /// The vsync event sender. Set when the engine subscribes via
    /// `vsync_events()`. The Kotlin side fires [`Self::fire_vsync`] via
    /// JNI when Choreographer ticks; that pushes a `()` into this channel.
    vsync_txs: Mutex<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
    /// Whether a vsync is currently armed. Idempotent guard — multiple
    /// `request_vsync` calls before the next fire coalesce into one.
    vsync_armed: std::sync::atomic::AtomicBool,
}

impl AndroidSchedulerDriver {
    /// Construct. Captures the tokio runtime handle + the JNI frame loop
    /// ref (or `None` for the runtime's base driver — no vsync, used only
    /// to spawn the worker + serve `sleep`). Sets up the main-thread
    /// `LocalPool` so `spawn_local` / `block_on` work on the calling
    /// (main) thread.
    pub fn new(runtime: tokio::runtime::Handle, frame_loop: Option<FrameLoopRef>) -> Rc<Self> {
        CURRENT_POOL.with(|c| *c.borrow_mut() = Some(LocalPool::new()));

        Rc::new(Self {
            inner: Arc::new(AndroidInner {
                vsync_txs: Mutex::new(Vec::new()),
                vsync_armed: std::sync::atomic::AtomicBool::new(false),
            }),
            runtime: Arc::new(runtime),
            frame_loop,
        })
    }

    /// Called from JNI (`nativePump`) when Kotlin's Choreographer fires.
    /// Pushes a vsync event into the subscribed channel + clears the
    /// `vsync_armed` flag so the next `request_vsync` re-arms.
    pub fn fire_vsync(&self) {
        self.inner
            .vsync_armed
            .store(false, std::sync::atomic::Ordering::Release);
        for tx in self.inner.vsync_txs.lock().unwrap().iter() {
            let _ = tx.unbounded_send(());
        }
    }
}

impl MainScheduler for AndroidSchedulerDriver {
    fn spawn_worker(
        &self,
        factory: Box<dyn FnOnce(Rc<dyn WorkerScheduler>) + Send + 'static>,
    ) -> WorkerHandle {
        // Dedicated OS thread — guarantees main ≠ worker. The driver sets
        // up the thread-local LocalPool on the worker thread before
        // invoking the factory; the worker view is a fresh driver-like
        // object holding the same tokio Handle (cloned, Send + Sync) but
        // no JNI state (workers don't need vsync).
        let runtime = self.runtime.clone();
        let join = std::thread::Builder::new()
            .name("tur-worker".into())
            .spawn(move || {
                CURRENT_POOL.with(|c| *c.borrow_mut() = Some(LocalPool::new()));
                let worker_view: Rc<dyn WorkerScheduler> =
                    Rc::new(AndroidWorkerScheduler { runtime });
                factory(worker_view);
                CURRENT_POOL.with(|c| *c.borrow_mut() = None);
            })
            .expect("failed to spawn tur worker thread");
        WorkerHandle::new(Box::new(move || {
            let _ = join.join();
        }))
    }

    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        spawn_local_on_current_thread(fut);
    }

    fn vsync_events(&self) -> VsyncEvents {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        self.inner.vsync_txs.lock().unwrap().push(tx);
        VsyncEvents(rx)
    }

    fn request_vsync(&self) {
        // Base driver (no frame loop) never schedules — instances install
        // their own driver via `TurApp::set_main_scheduler`.
        let Some(frame_loop) = self.frame_loop.as_ref() else {
            return;
        };
        // Idempotent: no-op if a vsync is already armed.
        if self
            .inner
            .vsync_armed
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        let vm = match crate::java_vm() {
            Some(vm) => vm,
            None => return,
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            tracing::warn!("scheduler: JNI attach failed for request_vsync");
            return;
        };
        let loop_obj = unsafe { JObject::from_raw(frame_loop.kotlin_loop.as_raw()) };
        if let Err(e) = env.call_method(&loop_obj, "scheduleVsync", "()V", &[]) {
            tracing::warn!("scheduler: scheduleVsync failed: {e}");
        }
    }

    fn sleep(&self, d: Duration) -> Sleep {
        tokio_sleep(self.runtime.clone(), d)
    }
}

impl WorkerScheduler for AndroidSchedulerDriver {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        spawn_local_on_current_thread(fut);
    }

    fn block_on(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        block_on_on_current_thread(fut);
    }

    fn sleep(&self, d: Duration) -> Sleep {
        tokio_sleep(self.runtime.clone(), d)
    }
}

/// Worker-side scheduler view. Constructed on each worker thread inside
/// `spawn_worker`. Holds the shared tokio Handle for `sleep` but no JNI
/// state (workers don't call vsync APIs).
struct AndroidWorkerScheduler {
    runtime: Arc<tokio::runtime::Handle>,
}

impl WorkerScheduler for AndroidWorkerScheduler {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        spawn_local_on_current_thread(fut);
    }

    fn block_on(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        block_on_on_current_thread(fut);
    }

    fn sleep(&self, d: Duration) -> Sleep {
        tokio_sleep(self.runtime.clone(), d)
    }
}
