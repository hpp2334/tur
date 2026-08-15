//! Android scheduling objects — one per role (JNI-backed).
//!
//! - [`AndroidVsyncSource`] — per-instance frame cadence: `request_frame`
//!   calls into Kotlin's `FrameLoop.scheduleVsync()` via JNI
//!   (Choreographer-backed). When the Choreographer fires, Kotlin calls
//!   `nativePump` which invokes [`AndroidVsyncSource::fire_vsync`],
//!   pushing an event into every subscribed channel.
//! - [`AndroidMainLoop`] — main-thread task spawner: tasks are held in a
//!   list polled from `AndroidInstance::pump_loop` (each wake-up), with
//!   wakers that request a prompt pump so pending tasks get their next
//!   poll. Roots the engine's main-thread drain (the
//!   `AsyncPluginContext` hop) + any embedder main-thread tasks.
//! - Worker hosting comes from
//!   [`tur_native::worker_pool::NativeWorkerPools`] (the shared native
//!   lane executor) with [`TokioLaneTimer`] as the lane timer — pools
//!   registered on the runtime are hosted on "tur-lane" threads,
//!   cooperatively scheduled when a pool's cap forces sharing, with
//!   dedicated-thread `spawn_blocking` offload.
//!
//! ## Frame loop ownership
//!
//! Each `AndroidInstance` owns its own Kotlin `FrameLoop` (Choreographer
//! callback), so the vsync source is **per-instance**:
//! `AndroidRuntime::build` installs a base source with no frame loop
//! (its `request_frame` no-ops — the worker thread is all it needs), and
//! each instance replaces the app's vsync source via
//! `TurApp::set_vsync_source` with one bound to its own `FrameLoop`.
//! Multiple subscribers per source are supported (broadcast).
//!
//! ## Timers
//!
//! Lane `sleep` is backed by `tokio::time::sleep` bridged via a oneshot.
//! The timer holds a `tokio::runtime::Handle` (shared with
//! `HttpBackend`) for **timers only**.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use jni::objects::JObject;
use tur_native::worker_pool::{LaneTimer, NativeWorkerPools};

use tur_engine::core::scheduler::{
    LocalFut, MainLoop, Sleep, TaskHandle, VsyncEvents, VsyncSource, track_spawn,
};

/// Handle to Kotlin's `org.tur.FrameLoop` object, stashed at create time so
/// the vsync source (which the engine calls from its own frame tick) can
/// reach it.
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

// ---------------------------------------------------------------------------
// Lane timer (worker-side sleep)
// ---------------------------------------------------------------------------

/// Lane timer backed by `tokio::time::sleep` bridged via a oneshot. Holds
/// the shared tokio runtime `Handle` for **timers only**.
pub struct TokioLaneTimer {
    runtime: Arc<tokio::runtime::Handle>,
}

impl LaneTimer for TokioLaneTimer {
    fn sleep(&self, d: Duration) -> Sleep {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.runtime.spawn(async move {
            tokio::time::sleep(d).await;
            let _ = tx.send(());
        });
        Sleep(Box::pin(async move {
            let _ = rx.await;
        }))
    }
}

/// Build the native worker host with the tokio-backed lane timer. The
/// handle is shared with `HttpBackend` (reqwest) by the embedder.
pub fn worker_host(runtime: tokio::runtime::Handle) -> Rc<NativeWorkerPools> {
    let runtime = Arc::new(runtime);
    Rc::new(NativeWorkerPools::with_timer(Arc::new(move || {
        Rc::new(TokioLaneTimer {
            runtime: runtime.clone(),
        })
    })))
}

// ---------------------------------------------------------------------------
// Vsync
// ---------------------------------------------------------------------------

struct AndroidVsyncInner {
    /// The vsync event senders. Set when the engine subscribes via
    /// `subscribe()`. The Kotlin side fires [`AndroidVsyncSource::fire_vsync`]
    /// via JNI when Choreographer ticks; that pushes a `()` into each
    /// channel.
    vsync_txs: Mutex<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
    /// Whether a vsync is currently armed. Idempotent guard — multiple
    /// `request_frame` calls before the next fire coalesce into one.
    vsync_armed: std::sync::atomic::AtomicBool,
}

/// Per-instance vsync source bound to the instance's Kotlin `FrameLoop`
/// (or `None` for the runtime's base source — no Choreographer, used only
/// to host workers + serve timers at runtime-build time).
pub struct AndroidVsyncSource {
    inner: Arc<AndroidVsyncInner>,
    /// JNI `FrameLoop` global ref. The source's `request_frame` calls
    /// `scheduleVsync()` on this object. `None` for the runtime base
    /// source (no frame loop at runtime-build time; instances install
    /// their own via `TurApp::set_vsync_source`).
    frame_loop: Option<FrameLoopRef>,
}

impl AndroidVsyncSource {
    /// Construct. `frame_loop` is `None` for the runtime's base source.
    pub fn new(frame_loop: Option<FrameLoopRef>) -> Rc<Self> {
        Rc::new(Self {
            inner: Arc::new(AndroidVsyncInner {
                vsync_txs: Mutex::new(Vec::new()),
                vsync_armed: std::sync::atomic::AtomicBool::new(false),
            }),
            frame_loop,
        })
    }

    /// Called from JNI (`nativePump`) when Kotlin's Choreographer fires.
    /// Pushes a vsync event into the subscribed channel + clears the
    /// `vsync_armed` flag so the next `request_frame` re-arms.
    pub fn fire_vsync(&self) {
        self.inner
            .vsync_armed
            .store(false, std::sync::atomic::Ordering::Release);
        for tx in self.inner.vsync_txs.lock().unwrap().iter() {
            let _ = tx.unbounded_send(());
        }
    }

    /// Returns a `Send + Sync` closure that requests a **message pump** —
    /// a coalesced main-Handler post that polls the loop WITHOUT firing a
    /// vsync. Used as the waker for `pump_loop` so that when the worker
    /// sends to `main_rx` (from its own thread), the channel waker fires
    /// this closure → the main thread polls the loop promptly.
    ///
    /// Deliberately does NOT arm the Choreographer: arming a vsync here
    /// would make every worker→main message (each pump ships a
    /// `FrameOutcome`) re-arm the next display frame, ping-ponging the
    /// whole engine (flush per pump) at display refresh rate forever —
    /// even fully idle. The Choreographer is armed ONLY by
    /// [`VsyncSource::request_frame`](Self::request_frame), i.e. by the
    /// engine's `FrameOutcome.schedule == Vsync` decision.
    pub fn make_vsync_wake_fn(&self) -> Arc<dyn Fn() + Send + Sync> {
        let Some(frame_loop) = self.frame_loop.as_ref() else {
            return Arc::new(|| {});
        };
        let kotlin_loop = frame_loop.kotlin_loop.clone();
        Arc::new(move || {
            let Some(vm) = crate::java_vm() else { return };
            let Ok(mut env) = vm.attach_current_thread() else {
                return;
            };
            let loop_obj = unsafe { JObject::from_raw(kotlin_loop.as_raw()) };
            let _ = env.call_method(&loop_obj, "requestPump", "()V", &[]);
        })
    }
}

impl VsyncSource for AndroidVsyncSource {
    fn subscribe(&self) -> VsyncEvents {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        self.inner.vsync_txs.lock().unwrap().push(tx);
        VsyncEvents(rx)
    }

    fn request_frame(&self) {
        // Base source (no frame loop) never schedules — instances install
        // their own via `TurApp::set_vsync_source`.
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
        let Some(vm) = crate::java_vm() else { return };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let loop_obj = unsafe { JObject::from_raw(frame_loop.kotlin_loop.as_raw()) };
        let _ = env.call_method(&loop_obj, "scheduleVsync", "()V", &[]);
    }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

/// A wake closure: arms a Choreographer vsync on a live instance.
pub type WakeFn = Arc<dyn Fn() + Send + Sync>;

/// Waker that fires the main loop's registered wake closures (requesting a
/// prompt pump on every live instance) so pending main-loop tasks get
/// polled on the next `pump_loop`.
struct MainLoopWaker(Arc<Mutex<Vec<WakeFn>>>);

impl std::task::Wake for MainLoopWaker {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        for f in self.0.lock().unwrap().iter() {
            f();
        }
    }
}

/// Main-thread task spawner for Android: tasks are held in a list and
/// polled cooperatively from `AndroidInstance::pump_loop`. Task wakers
/// request a pump (via the wake closures registered by live instances) so
/// a task that becomes ready between pumps gets polled promptly.
///
/// This roots the engine's main-thread drain (the `AsyncPluginContext`
/// hop — clipboard `run_on_main` etc.), which historically sat on an
/// unpumped `LocalPool` and could never advance.
pub struct AndroidMainLoop {
    tasks: Rc<RefCell<Vec<LocalFut>>>,
    wake_fns: Arc<Mutex<Vec<WakeFn>>>,
}

impl AndroidMainLoop {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            tasks: Rc::new(RefCell::new(Vec::new())),
            wake_fns: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Register a wake closure — typically each instance's message-pump
    /// request fn (`AndroidVsyncSource::make_vsync_wake_fn`), so any
    /// pending main-loop task schedules a pump on a live instance and
    /// thereby gets polled.
    pub fn add_wake_fn(&self, f: WakeFn) {
        self.wake_fns.lock().unwrap().push(f);
    }

    fn fire_wakes(&self) {
        for f in self.wake_fns.lock().unwrap().iter() {
            f();
        }
    }

    /// Poll every live task once (one cooperative pass). Called from
    /// `AndroidInstance::pump_loop` after the frame loop's own poll.
    /// Completed tasks are removed; pending tasks re-arm via their waker
    /// (which requests the next pump).
    pub fn poll(&self) {
        let waker = std::task::Waker::from(Arc::new(MainLoopWaker(self.wake_fns.clone())));
        let mut cx = std::task::Context::from_waker(&waker);
        let mut tasks = self.tasks.borrow_mut();
        let mut i = 0;
        while i < tasks.len() {
            if tasks[i].as_mut().poll(&mut cx).is_ready() {
                drop(tasks.remove(i));
            } else {
                i += 1;
            }
        }
    }
}

impl MainLoop for AndroidMainLoop {
    fn spawn_local(&self, fut: LocalFut) -> TaskHandle {
        let handle = track_spawn(fut, |tracked| {
            self.tasks.borrow_mut().push(tracked);
        });
        // Request a pump so the task gets its first poll promptly.
        self.fire_wakes();
        handle
    }
}
