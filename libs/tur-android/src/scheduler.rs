//! Android scheduler driver (JNI-backed).
//!
//! Implements [`MainSchedulerDriver`] for Android. Worker spawning goes
//! through [`tur_native::worker_pool::NativeWorkerPools`] (the shared
//! native lane executor): pools registered on the runtime are hosted on
//! "tur-lane" threads, cooperatively scheduled when a pool's cap forces
//! sharing. The lane driver provides `sleep` via the shared tokio runtime.
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

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use futures::executor::{LocalPool, LocalSpawner};
use futures::task::LocalSpawnExt;
use jni::objects::JObject;
use tur_native::worker_pool::NativeWorkerPools;

use tur_engine::core::scheduler::{
    MainSchedulerDriver, Sleep, TaskHandle, VsyncEvents, WorkerFactory, WorkerHandle,
    WorkerPoolHandle, WorkerSchedulerDriver, track_spawn,
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
    /// Per-thread `LocalPool` for `run_until` (drives the worker_loop).
    /// Borrowed **mutably** for the entire duration of `run_until`.
    static CURRENT_POOL: RefCell<Option<LocalPool>> = const { RefCell::new(None) };
    /// Per-thread `LocalSpawner` (extracted from the pool at setup time).
    /// Stored separately so `spawn_local` can borrow it while `run_until`
    /// holds the mutable borrow on `CURRENT_POOL` — without this, a future
    /// polled inside `run_until` that calls `spawn_local` panics with
    /// "RefCell already mutably borrowed".
    static CURRENT_SPAWNER: RefCell<Option<LocalSpawner>> = const { RefCell::new(None) };
}

/// Set up the current thread's `LocalPool` + `LocalSpawner`. Called once
/// per thread (main thread in `AndroidSchedulerDriver::new`, worker thread
/// in `spawn_worker`).
fn set_up_thread_pool() {
    CURRENT_POOL.with(|c| {
        let pool = LocalPool::new();
        CURRENT_SPAWNER.with(|s| *s.borrow_mut() = Some(pool.spawner()));
        *c.borrow_mut() = Some(pool);
    });
}

/// Helper: spawn a future on the current thread's LocalSpawner.
fn spawn_local_on_current_thread(fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
    CURRENT_SPAWNER.with(|s| {
        let guard = s.borrow();
        let spawner = guard
            .as_ref()
            .expect("spawn_local called with no LocalPool set on this thread");
        track_spawn(fut, |f| {
            let _ = spawner.spawn_local(f);
        })
    })
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
    /// Native lane-pool registry backing `spawn_worker_in` (main-thread
    /// only — spawns happen from `app_builder().build()` on main).
    pools: Rc<NativeWorkerPools>,
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
        set_up_thread_pool();

        Rc::new(Self {
            inner: Arc::new(AndroidInner {
                vsync_txs: Mutex::new(Vec::new()),
                vsync_armed: std::sync::atomic::AtomicBool::new(false),
            }),
            runtime: Arc::new(runtime),
            frame_loop,
            pools: Rc::new(NativeWorkerPools::new()),
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

    /// Returns a `Send + Sync` closure that schedules a Choreographer vsync.
    /// Used by `pump_loop`'s waker so that when the worker sends to `main_rx`
    /// (from its own thread), the channel waker fires this closure → arms a
    /// vsync → Choreographer fires → `pump_loop` → processes the message.
    pub fn make_vsync_wake_fn(&self) -> Arc<dyn Fn() + Send + Sync> {
        let Some(frame_loop) = self.frame_loop.as_ref() else {
            return Arc::new(|| {});
        };
        let kotlin_loop = frame_loop.kotlin_loop.clone();
        let inner = self.inner.clone();
        Arc::new(move || {
            if inner
                .vsync_armed
                .swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                return;
            }
            let Some(vm) = crate::java_vm() else { return };
            let Ok(mut env) = vm.attach_current_thread() else {
                return;
            };
            let loop_obj = unsafe { JObject::from_raw(kotlin_loop.as_raw()) };
            let _ = env.call_method(&loop_obj, "scheduleVsync", "()V", &[]);
        })
    }
}

impl MainSchedulerDriver for AndroidSchedulerDriver {
    fn spawn_worker_in(&self, pool: &WorkerPoolHandle, factory: WorkerFactory) -> WorkerHandle {
        // Host the app on a tur-native lane thread (dedicated while the
        // pool is under its cap, shared cooperatively beyond it). The lane
        // driver serves `sleep` via the shared tokio runtime handle.
        let runtime = self.runtime.clone();
        self.pools.spawn(
            pool,
            factory,
            Arc::new(move || {
                Rc::new(AndroidWorkerScheduler {
                    runtime: runtime.clone(),
                })
            }),
        )
    }

    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        spawn_local_on_current_thread(fut)
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

/// Lane-thread scheduling driver: serves `sleep` via the shared tokio
/// runtime (the lane executor provides `spawn_local` itself, so the impl
/// here is unreachable through the engine).
struct AndroidWorkerScheduler {
    runtime: Arc<tokio::runtime::Handle>,
}

impl WorkerSchedulerDriver for AndroidWorkerScheduler {
    fn spawn_local(&self, _fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        unimplemented!(
            "lane spawn_local is provided by tur_native's lane executor; \
             AndroidWorkerScheduler only serves `sleep`"
        )
    }

    fn sleep(&self, d: Duration) -> Sleep {
        tokio_sleep(self.runtime.clone(), d)
    }
}
