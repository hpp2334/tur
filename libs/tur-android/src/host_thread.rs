//! The **tur-host thread** — the single OS thread that owns every piece of
//! `!Send` Android embedder state, so the Android main (UI) thread never
//! runs engine work.
//!
//! What lives here (created/held in [`HostState`]):
//! - the [`AndroidRuntime`] (its `Rc<TurRuntime>`, the `Rc<AndroidHostLoop>`
//!   task list, the `NativeWorkerPools` lane registry, the wgpu `Instance`),
//! - every [`AndroidInstance`] (`Rc<TurApp>`, the `!Send`
//!   `TurAppLooper::run` future, the wgpu `VelloRenderer` — GPU encode +
//!   present happen here, not on main),
//! - the engine's host-side drain (`AndroidHostLoop::poll`), which the
//!   scheduler contract only requires to be polled on **one consistent**
//!   thread — not Android's main looper specifically.
//!
//! ## How work reaches the thread
//!
//! Every JNI op is a thin marshalled closure ([`HostOp::Run`]) over
//! `&mut HostState`. The op channel is a FIFO, which gives the whole embedder
//! total ordering for free: an op posted after `createInstance` can never
//! observe a half-built instance, and `destroy` always lands after the
//! create it follows. The channel doubles as the thread's park — it blocks
//! in `recv()` between ops, so an idle engine costs nothing.
//!
//! Fire-and-forget ops ([`HostHandle::post`]) cover the hot paths (per-frame
//! `pump`, input, resize); blocking round-trips ([`HostHandle::call`]) cover
//! ops that must return (`with_app` / `with_runtime`, the runtime build).
//!
//! ## Wake path (no main-thread hop)
//!
//! Cross-thread wakes (worker→host messages, host-loop task wakers) used to
//! detour through Kotlin — JNI `FrameLoop.requestPump()` → main-Handler post
//! → `pumpMessages` JNI → poll. They now post a poll-only pump op **directly**
//! onto this thread's queue: a channel send, no JNI, no Android main-thread
//! involvement. The Choreographer (still on main, where
//! `SurfaceHolder.Callback` and input dispatch arrive) remains the *only*
//! main-thread touchpoint: its callback is a trivial `post` of the vsync
//! pump.
//!
//! ## Panic policy
//!
//! Every op runs under `catch_unwind`; a panic is logged (payload + full
//! backtrace land in logcat via the crate's panic hook) and the process
//! aborts — the same policy the old JNI-boundary `pump` had, kept because
//! resuming after a half-finished frame could leave the `!Send` state
//! inconsistent.
//!
//! The module compiles on non-Android targets too (against `app`'s empty
//! stubs) so the desktop `cargo check` covers it; it is never used there.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use crate::ModuleSourceRegistry;
use crate::app::{AndroidInstance, AndroidRuntime};

/// Global instance-id allocator. Monotonic and never reused, so a stale
/// wake (e.g. a Choreographer tick racing `destroy`) resolves to "no such
/// instance" instead of a different instance.
static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate the next instance id (called on the JNI thread when
/// `createInstance` hands the handle back, before the host-thread build
/// runs — the id is what later ops address the slot by).
pub(crate) fn next_instance_id() -> u64 {
    NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Everything the host thread owns. Only ever touched from inside an op
/// closure, on the host thread itself.
pub(crate) struct HostState {
    runtime: Option<Box<AndroidRuntime>>,
    instances: HashMap<u64, Box<AndroidInstance>>,
}

impl HostState {
    fn new() -> Self {
        Self {
            runtime: None,
            instances: HashMap::new(),
        }
    }

    /// Install the freshly built runtime (`createRuntime`).
    pub(crate) fn set_runtime(&mut self, runtime: Box<AndroidRuntime>) {
        self.runtime = Some(runtime);
    }

    /// The runtime (absent if destroyed, or — briefly — never built).
    pub(crate) fn runtime(&self) -> Option<&AndroidRuntime> {
        self.runtime.as_deref()
    }

    /// Look up a live instance by slot id.
    pub(crate) fn instance(&self, id: u64) -> Option<&AndroidInstance> {
        self.instances.get(&id).map(|b| &**b)
    }

    /// Insert a freshly built instance (`createInstance` op).
    pub(crate) fn insert_instance(&mut self, id: u64, instance: Box<AndroidInstance>) {
        self.instances.insert(id, instance);
    }

    /// Take an instance out for teardown (`destroy`). The `Box` drops (and
    /// with it the `Rc<TurApp>`, renderer, and worker ticket) on the host
    /// thread.
    pub(crate) fn remove_instance(&mut self, id: u64) -> Option<Box<AndroidInstance>> {
        self.instances.remove(&id)
    }

    /// Shutdown path: drop any leftover instances, then the runtime. Called
    /// from the `destroyRuntime` op (the Kotlin contract says destroy
    /// instances first; this is the defensive backstop).
    pub(crate) fn clear_all(&mut self) {
        let leftover = self.instances.len();
        if leftover > 0 {
            tracing::info!("tur-host: dropping {leftover} leftover instance(s) at runtime destroy");
        }
        self.instances.clear();
        self.runtime.take();
    }
}

/// Loop control returned by an op closure: [`Flow::Continue`] parks the
/// thread on the next op; [`Flow::Stop`] exits the loop (the
/// `destroyRuntime` shutdown path — the thread finishes regardless of any
/// sender clones still held by stale routes or in-flight wakes).
pub(crate) enum Flow {
    Continue,
    Stop,
}

/// One marshalled unit of work. The closure receives `&mut HostState` —
/// exclusive access to every `!Send` object the embedder owns — and is
/// `Send` because it crossed a thread boundary to get here.
pub(crate) enum HostOp {
    Run(Box<dyn FnOnce(&mut HostState) -> Flow + Send>),
}

/// `Send + Clone` posting handle onto the host thread — what every JNI op,
/// route, and cross-thread wake fn holds. The channel provides both the FIFO
/// op queue and the thread's park.
#[derive(Clone)]
pub(crate) struct HostHandle {
    tx: mpsc::Sender<HostOp>,
}

impl HostHandle {
    /// Fire-and-forget: run `f` on the host thread at its place in the
    /// queue. Returns `false` if the thread has shut down (op dropped).
    pub(crate) fn post(&self, f: impl FnOnce(&mut HostState) + Send + 'static) -> bool {
        self.post_flow(move |state| {
            f(state);
            Flow::Continue
        })
    }

    /// [`post`](Self::post) with loop control — the shutdown path returns
    /// [`Flow::Stop`].
    pub(crate) fn post_flow(
        &self,
        f: impl FnOnce(&mut HostState) -> Flow + Send + 'static,
    ) -> bool {
        self.tx.send(HostOp::Run(Box::new(f))).is_ok()
    }

    /// Blocking round-trip: run `f` on the host thread and wait for its
    /// result. FIFO order still applies — the call lands behind every op
    /// posted before it, so it observes all earlier effects.
    pub(crate) fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut HostState) -> T + Send + 'static,
    ) -> Result<T, &'static str> {
        let (tx, rx) = mpsc::channel();
        if !self.post_flow(move |state| {
            let _ = tx.send(f(state));
            Flow::Continue
        }) {
            return Err("tur-host thread has shut down");
        }
        rx.recv().map_err(|_| "tur-host thread exited mid-call")
    }
}

/// What Kotlin's **runtime** `jlong` handle points at — allocated on the JNI
/// thread in `createRuntime`, freed in `destroyRuntime`. Carries everything
/// a caller-thread op needs *without* a host round-trip (the Arc-shared
/// module-source registry) plus the routing to the host thread and the join
/// handle used at teardown.
pub struct RuntimeRoute {
    pub(crate) host: HostHandle,
    /// Arc-shared with the host-thread `AndroidRuntime`'s registry — same
    /// entries on both halves, so `registerModuleSource` /
    /// `releaseModuleSource` (and Rust-side registrations via
    /// [`crate::ops::with_runtime`]) work from any thread.
    pub(crate) module_sources: ModuleSourceRegistry,
    /// Join handle, taken + joined by `destroyRuntime`.
    pub(crate) join: Option<JoinHandle<()>>,
}

/// What Kotlin's **instance** `jlong` handle points at — allocated on the
/// JNI thread in `createInstance` (so the handle is valid immediately,
/// before the heavy host-thread build runs), freed in `destroy`. `id`
/// addresses the host-thread slot; ops against an id whose build failed or
/// whose instance was destroyed are ordered no-ops.
pub struct InstanceRoute {
    pub(crate) host: HostHandle,
    pub(crate) id: u64,
}

/// Spawn the host thread for one runtime. Returns the posting handle (what
/// routes and wake fns hold) and the join handle (stashed in the
/// [`RuntimeRoute`], joined at `destroyRuntime`).
pub(crate) fn spawn() -> (HostHandle, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<HostOp>();
    let join = std::thread::Builder::new()
        .name("tur-host".to_string())
        .spawn(move || {
            // Attach to the JVM permanently (not per-call): this thread
            // now makes JNI calls on the hot path (vsync arm, text-input
            // push) and drops `GlobalRef`s (Kotlin object refs), and the
            // implicit attach/detach a bare thread would do per call is
            // both slow and noisy in ART diagnostics.
            if let Some(vm) = crate::java_vm()
                && let Ok(mut env) = vm.attach_current_thread_permanently()
            {
                // ART renames attached native threads to "Thread-N",
                // clobbering the Rust-side "tur-host" name. Restore it via
                // `Thread.currentThread().setName("tur-host")` (ART maps
                // setName to prctl(PR_SET_NAME), so the name lands in
                // /proc/<tid>/comm — what `top -H`, systrace, and
                // tombstones attribute work by).
                if let Ok(name) = env.new_string("tur-host")
                    && let Ok(thread) = env.call_static_method(
                        "java/lang/Thread",
                        "currentThread",
                        "()Ljava/lang/Thread;",
                        &[],
                    )
                    && let Ok(thread_obj) = thread.l()
                {
                    let _ = env.call_method(
                        &thread_obj,
                        "setName",
                        "(Ljava/lang/String;)V",
                        &[jni::objects::JValue::Object(&name)],
                    );
                }
            }
            let mut state = HostState::new();
            while let Ok(HostOp::Run(f)) = rx.recv() {
                // Panic policy (see module docs): the hook has already
                // logged payload + backtrace to logcat; abort rather than
                // resume with half-finished `!Send` state.
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut state))) {
                    Ok(Flow::Continue) => {}
                    // Deterministic shutdown: the stop op ran to completion
                    // (its effects — dropped instances, dropped runtime —
                    // are visible), so exit even if sender clones (stale
                    // instance routes in Kotlin) are still alive. Without
                    // this the loop would park in `recv()` forever and
                    // `destroyRuntime`'s join would deadlock.
                    Ok(Flow::Stop) => break,
                    Err(_) => {
                        tracing::error!("tur-host: op panicked — aborting");
                        std::process::abort();
                    }
                }
            }
        })
        .expect("failed to spawn tur-host thread");
    (HostHandle { tx }, join)
}
