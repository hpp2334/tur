# Plan: `Scheduler` refactor

**Branch**: `refactor/scheduler` (off `refactor/thread-v2`)
**Goal**: Replace `LoopDriver` + `AsyncExecutor` + `wasm_thread` dep with a unified platform-implemented `SchedulerDriver` trait. Hard break — old APIs deleted on the branch.

---

## 1. Design summary

The engine becomes **event-driven**. It exposes one async method `start_loop` that the embedder spawns on its platform's runtime. The engine subscribes to a vsync event stream from the driver + sends `WorkerMsg::Wake` to the worker when vsync fires; the worker pumps, ships render commands + `FrameOutcome` back to main; main applies the render sink + (if `outcome.schedule == Vsync`) fire-and-forget `request_vsync()` for the next frame. When idle, main just blocks on `events.next().await`.

**Dependency direction is one-way**: engine → scheduler. The driver has zero knowledge of the engine (no `set_wake` callback registration).

### Three driver impls

Each driver implements **both** `MainScheduler` and `WorkerScheduler` traits. The runtime holds two separate trait objects (`Rc<dyn MainScheduler>` + `Rc<dyn WorkerScheduler>`), both pointing at the same underlying driver. The `.scheduler(driver)` convenience method on the builder sets both from a single `Rc<S>` where `S: MainScheduler + WorkerScheduler`.

| Driver | Location | Notes |
|---|---|---|
| `WasmSchedulerDriver` | tur-wasm (new `scheduler.rs`) | wasm_thread for spawn_worker; rAF + setTimeout; no tokio |
| `AndroidSchedulerDriver` | tur-android (new `scheduler.rs`, replaces `loop_driver.rs`) | tokio Handle for **timers only**; std::thread for spawn_worker; JNI Choreographer for vsync |
| `TestSchedulerDriver` | tur-integration-tests (new `test_scheduler.rs`) | Real std::thread per worker + virtual clock via Condvar + BinaryHeap; deterministic AND thread-faithful |

---

## 2. New module `libs/tur-engine/src/core/scheduler.rs`

Two separate traits — `MainScheduler` (main-thread surface) and `WorkerScheduler` (worker-thread surface). Both implemented by the same driver object; the runtime holds two `Rc` trait objects pointing at it.

```rust
/// Main-thread scheduling surface. Used by `TurApp`'s autonomous loop
/// and by `MainBackend::new` to spawn workers. Methods here are valid
/// only when called from the main thread.
pub trait MainScheduler: 'static {
    /// Spawn a worker. The factory runs on a new worker thread
    /// (`std::thread` on native, Web Worker via `wasm_thread` on wasm).
    /// The factory is `FnOnce() -> R` — it captures whatever it needs
    /// from the outside (typically the runtime's `WorkerScheduler` clone).
    /// The driver is responsible for setting up any thread-locals
    /// (e.g. the LocalPool) on the worker thread *before* invoking the
    /// factory.
    fn spawn_worker<F, R>(&self, factory: F) -> WorkerHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static;

    /// Spawn a future on the main thread's local executor.
    fn spawn_local<F>(&self, fut: F)
    where F: Future<Output = ()> + 'static;

    /// Subscribe to vsync events. Each item is one vsync tick.
    /// Call once at engine startup. Events only fire when armed via
    /// `request_vsync`.
    fn vsync_events(&self) -> VsyncEvents;

    /// Arm the next vsync. Idempotent — multiple calls before the next
    /// vsync are coalesced into one rAF/Choreographer request (fixes
    /// the rAF churn perf bug as a side effect).
    fn request_vsync(&self);

    /// Create a Sleep future. Implementation is platform-specific.
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Worker-thread scheduling surface. Held by `WorkerBackend`; bridges
/// grab it from `PluginContext` / `SubsystemFlushContext`. Methods here
/// dispatch to the *current thread's* executor (thread-local LocalPool).
pub trait WorkerScheduler: 'static {
    /// Spawn a future on this worker thread's local executor.
    fn spawn_local<F>(&self, fut: F)
    where F: Future<Output = ()> + 'static;

    /// Block the calling (worker) thread on a future. Drives both the
    /// future AND any spawn_local'd side-futures (LocalPool semantics).
    fn block_on<F, R>(&self, fut: F) -> R
    where F: Future<Output = R>;

    /// Create a Sleep future.
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Newtype around a boxed future. Drivers construct it from their
/// platform-specific timer primitive; consumers just `.await` it.
pub struct Sleep(pub Pin<Box<dyn Future<Output = ()> + 'static>>);
impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.0.as_mut().poll(cx)
    }
}

pub struct VsyncEvents(futures::channel::mpsc::UnboundedReceiver<()>);
impl Stream for VsyncEvents { /* delegates to .0 */ }

/// Returned by spawn_worker. Main holds it for the worker's lifetime.
pub struct WorkerHandle<R: Send + 'static> { join: Box<dyn FnOnce() -> R> }
impl<R: Send + 'static> WorkerHandle<R> {
    pub fn join(self) -> R { (self.join)() }
}
```

**Why split into two traits:**
- Clear API surface — `MainScheduler` methods are main-only, `WorkerScheduler` methods are worker-only. Impossible to call `block_on` from main by accident.
- The runtime holds two distinct trait objects, making the dependency direction explicit at the type level.
- The same driver object implements both; thread-locals inside the impl dispatch to the right per-thread state.

**Thread-local dispatch (per driver impl):**

Each driver uses thread-local storage to track the current thread's `LocalPool`. The driver's `spawn_worker` impl sets up the worker thread's LocalPool before invoking the factory:

```rust
// Pseudocode for AndroidSchedulerDriver
thread_local! {
    static CURRENT_POOL: RefCell<Option<Rc<LocalPool>>> = RefCell::new(None);
}

impl MainScheduler for AndroidSchedulerDriver {
    fn spawn_worker<F, R>(&self, factory: F) -> WorkerHandle<R>
    where F: FnOnce() -> R + Send + 'static, R: Send + 'static
    {
        std::thread::Builder::new()
            .name("tur-worker".into())
            .spawn(move || {
                // Set up thread-local LocalPool for this worker thread.
                let pool = Rc::new(futures::executor::LocalPool::new());
                CURRENT_POOL.with(|c| *c.borrow_mut() = Some(pool.clone()));
                let result = factory();
                CURRENT_POOL.with(|c| *c.borrow_mut() = None);
                result
            })
            .unwrap()
        // ... wrap in WorkerHandle
    }

    fn spawn_local<F: Future<Output=()> + 'static>(&self, fut: F) {
        // Main-thread version. Main's LocalPool is set up at embedder startup.
        CURRENT_POOL.with(|pool| pool.borrow().as_ref().unwrap().spawner().spawn_local(fut));
    }
    // ...
}

impl WorkerScheduler for AndroidSchedulerDriver {
    fn spawn_local<F: Future<Output=()> + 'static>(&self, fut: F) {
        CURRENT_POOL.with(|pool| pool.borrow().as_ref().unwrap().spawner().spawn_local(fut));
    }
    fn block_on<F, R>(&self, fut: F) -> R {
        CURRENT_POOL.with(|pool| pool.borrow_mut().as_ref().unwrap().run_until(fut))
    }
    // ...
}
```

The embedder is responsible for setting up the main thread's LocalPool at startup (one line in `WasmApp::create`, `AndroidInstance::build_with_surface`, etc.).

---

## 3. Engine internals

### `MainBackend::new` (rewrite)

```rust
pub(crate) fn new(
    main: Rc<dyn MainScheduler>,
    worker: Rc<dyn WorkerScheduler>,
    factory: impl FnOnce(Rc<dyn WorkerScheduler>) -> WorkerBackend + Send + 'static,
) -> Self {
    let worker_clone = worker.clone();
    let worker_handle = main.spawn_worker(move || {
        // Inside this closure, we're on the worker thread. The driver
        // has already set up the thread-local LocalPool before invoking
        // us. The worker_clone dispatches via thread-local to that pool.
        let backend = factory(worker_clone.clone());
        worker_clone.block_on(worker_loop(backend, worker_rx, main_tx))
    });
    // ... (drop core::thread import + native init-signal dance)
}
```

### `TurApp::start_loop` (replaces `start` + `pump` + `spawn_wake`)

```rust
pub async fn start_loop(self: Rc<Self>) {
    assert!(!self.loop_started.replace(true), "start_loop called twice");

    let mut vsync_rx = self.main_sched.vsync_events();
    let mut main_rx = self.backend.main_rx.borrow_mut();

    loop {
        // Merge vsync + main_msg streams. select() from futures crate.
        let next_event = futures::stream::select(
            (&mut vsync_rx).map(|_| Ev::Vsync),
            (&mut main_rx).map(Ev::MainMsg),
        );

        match next_event.next().await {
            Some(Ev::Vsync) => {
                if self.destroyed.get() { break; }
                self.backend.send_worker_msg(WorkerMsg::Wake);
            }
            Some(Ev::MainMsg(msg)) => {
                if self.handle_main_msg(msg).should_stop() { break; }
            }
            None => break,
        }
    }
}

fn handle_main_msg(&self, msg: MainMsg) -> HandleResult {
    match msg {
        MainMsg::RenderCommands { commands, image_map, viewport } => {
            self.apply_render_sink(commands, image_map, viewport);
        }
        MainMsg::FrameOutcome(outcome) => {
            if let Some(hook) = self.after_frame.borrow().as_ref().cloned() {
                hook(outcome.clone());
            }
            if outcome.schedule == NextFrame::Vsync {
                self.main_sched.request_vsync();   // fire-and-forget, idempotent
            }
            // Idle: no-op. Loop will block on next().await.
        }
        MainMsg::CursorChanged(c) => self.update_cursor(c),
        MainMsg::FocusedStateChanged { .. } => self.update_focus(..),
        MainMsg::Destroyed => return HandleResult::Stop,
        _ => {}
    }
    HandleResult::Continue
}
```

**Bootstrap**: `runtime.create_app(...)` always pushes an initial `PlatformEvent::Resize` to the worker. That triggers a worker pump → `RenderCommands` + `FrameOutcome` flow back to main → main requests next vsync. **No initial `request_vsync` needed.**

### `push_platform_event` (rewritten — no main-side wake)

```rust
pub fn push_platform_event(&self, event: PlatformEvent) {
    self.backend.send_worker_msg(WorkerMsg::PlatformEvent(event));
    self.backend.send_worker_msg(WorkerMsg::Wake);  // worker pumps immediately
}
```

Main is just a passive event receiver; worker gets the wake directly.

### `TurApp` fields (much smaller)

```rust
pub struct TurApp {
    backend: MainBackend,
    main_sched: Rc<dyn MainScheduler>,
    after_frame: RefCell<Option<AfterFrameHook>>,
    loop_started: Cell<bool>,
    destroyed: Cell<bool>,
}
```

No `pump_in_progress` / `wake_pending` / `in_flight` / `WakeNotify` — the loop itself is the serialization boundary.

### `NextFrame` — drop `After(d)` variant

```rust
pub enum NextFrame {
    Idle,
    Vsync,
    // After(d) removed — Sleep drives its own wake via CompletionHandle::on_push
}
```

### `CompletionQueue` replaces `AsyncExecutor`

In `libs/tur-engine/src/core/async_/completion.rs` (new):

```rust
pub struct CompletionQueue {
    pending: Rc<RefCell<VecDeque<Completion>>>,
    on_push: Rc<dyn Fn()>,  // sends WorkerMsg::Wake to the worker
}
pub struct CompletionHandle {
    pending: Rc<RefCell<VecDeque<Completion>>>,
    on_push: Rc<dyn Fn()>,
}
impl CompletionHandle {
    pub fn push(&self, f: Completion) {
        self.pending.borrow_mut().push_back(f);
        (self.on_push)();   // self-send Wake — worker flushes to drain
    }
}
```

### Worker self-wake on completion

`WorkerBackend` captures `worker_tx` clone at construction:

```rust
let worker_tx_clone = worker_tx.clone();
let completion_queue = CompletionQueue::new(Rc::new(move || {
    let _ = worker_tx_clone.unbounded_send(WorkerMsg::Wake);
}));
```

Self-send is sound — `futures::channel::mpsc` allows it. The message queues, next `worker_rx.next().await` resolves, worker pumps, drains completions, JsPromises settle.

### Bridges rewritten

All call sites change from:
```rust
executor.spawn_detached(async move { ... executor.complete(closure); });
```
to:
```rust
worker_sched.spawn_local(async move { ... completion_handle.push(closure); });
```

Affected:
- `libs/tur-engine/src/builtin_plugins/clipboard/bridge.rs`
- `libs/tur-engine/src/builtin_plugins/clipboard/handlers.rs`
- `libs/tur-engine/src/builtin_plugins/text/elements/editable_text/element.rs` (caret blink)
- `libs/tur-net-capability/src/bridge.rs`
- `libs/tur-filepicker-capability/src/bridge.rs`

`PluginContext` and `SubsystemFlushContext` grow fields to carry both `worker_sched: Rc<dyn WorkerScheduler>` and `completion_handle: CompletionHandle`.

### Sleep callsites

Any `executor.sleep(d)` becomes `worker_sched.sleep(d)` (delegating to `WorkerScheduler::sleep`, which dispatches to the driver's `sleep` implementation).

---

## 4. Driver impls

Each driver implements **both** `MainScheduler` and `WorkerScheduler`. The runtime casts/stores it as two separate trait objects (`Rc<dyn MainScheduler>` + `Rc<dyn WorkerScheduler>`), both pointing at the same underlying driver.

### `WasmSchedulerDriver` (tur-wasm, new `scheduler.rs`)

```rust
pub struct WasmSchedulerDriver {
    inner: Rc<WasmInner>,
}
struct WasmInner {
    vsync_tx: RefCell<Option<futures::channel::mpsc::UnboundedSender<()>>>,
    raf_id: Cell<Option<i32>>,
    raf_closure: Closure<dyn Fn()>,  // driver-owned; pushes event on fire
}

impl WasmSchedulerDriver {
    pub fn new() -> Self { /* construct raf_closure that calls inner.fire_vsync */ }
}

impl MainScheduler for WasmSchedulerDriver {
    fn spawn_worker<F, R>(&self, factory: F) -> WorkerHandle<R>
    where F: FnOnce() -> R + Send + 'static, R: Send + 'static
    {
        wasm_thread::spawn(move || {
            // Worker thread-local setup not needed on wasm —
            // wasm_bindgen_futures::spawn_local uses the JS event loop
            // directly. block_on uses futures::executor::block_on which
            // parks the worker thread (allowed on wasm workers).
            factory()
        });
        // Wrap join handle in WorkerHandle...
    }

    fn spawn_local<F: Future<Output = ()> + 'static>(&self, fut: F) {
        wasm_bindgen_futures::spawn_local(fut);
    }

    fn vsync_events(&self) -> VsyncEvents {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        *self.inner.vsync_tx.borrow_mut() = Some(tx);
        VsyncEvents(rx)
    }

    fn request_vsync(&self) {
        if self.inner.raf_id.get().is_some() { return; }   // idempotent
        let id = window.request_animation_frame(...).unwrap();
        self.inner.raf_id.set(Some(id));
    }

    fn sleep(&self, d: Duration) -> Sleep { /* setTimeout-backed via oneshot — see §3 */ }
}

impl WorkerScheduler for WasmSchedulerDriver {
    fn spawn_local<F: Future<Output = ()> + 'static>(&self, fut: F) {
        wasm_bindgen_futures::spawn_local(fut);
    }
    fn block_on<F, R>(&self, fut: F) -> R {
        futures::executor::block_on(fut)
    }
    fn sleep(&self, d: Duration) -> Sleep { /* same as MainScheduler impl */ }
}

impl WasmInner {
    fn fire_vsync(&self) {
        // rAF closure body — invoked by browser on vsync
        self.raf_id.set(None);
        if let Some(tx) = self.vsync_tx.borrow().as_ref() {
            let _ = tx.unbounded_send(());
        }
    }
}
```

`tur-wasm/Cargo.toml`: add `wasm_thread = { workspace = true }`.

### `AndroidSchedulerDriver` (tur-android, new `scheduler.rs`, replaces `loop_driver.rs`)

```rust
pub struct AndroidSchedulerDriver {
    runtime: tokio::runtime::Handle,   // shared with HttpBackend
    frame_loop: FrameLoopRef,          // JNI Choreographer
    inner: Rc<AndroidInner>,
    main_pool: Rc<futures::executor::LocalPool>,  // main-thread pool
}
struct AndroidInner {
    vsync_tx: RefCell<Option<UnboundedSender<()>>>,
    vsync_requested: AtomicBool,
}

thread_local! {
    static CURRENT_POOL: RefCell<Option<Rc<LocalPool>>> = RefCell::new(None);
}

impl AndroidSchedulerDriver {
    pub fn new(runtime: tokio::runtime::Handle, frame_loop: FrameLoopRef) -> Self {
        let driver = Self { runtime, frame_loop, inner: ..., main_pool: Rc::new(LocalPool::new()) };
        // Set up main-thread LocalPool.
        CURRENT_POOL.with(|c| *c.borrow_mut() = Some(driver.main_pool.clone()));
        driver
    }
    /// Called from JNI (nativePump) when Choreographer fires.
    pub fn fire_vsync(&self) { self.inner.fire_vsync(); }
}

impl MainScheduler for AndroidSchedulerDriver {
    fn spawn_worker<F, R>(&self, factory: F) -> WorkerHandle<R>
    where F: FnOnce() -> R + Send + 'static, R: Send + 'static
    {
        // Dedicated OS thread — guarantees main ≠ worker.
        let join = std::thread::Builder::new()
            .name("tur-worker".into())
            .spawn(move || {
                // Set up thread-local LocalPool for this worker thread.
                let pool = Rc::new(futures::executor::LocalPool::new());
                CURRENT_POOL.with(|c| *c.borrow_mut() = Some(pool.clone()));
                let result = factory();
                CURRENT_POOL.with(|c| *c.borrow_mut() = None);
                result
            })
            .unwrap();
        WorkerHandle { join: Box::new(move || join.join().unwrap()) }
    }

    fn spawn_local<F: Future<Output = ()> + 'static>(&self, fut: F) {
        CURRENT_POOL.with(|pool| {
            pool.borrow().as_ref().unwrap().spawner().spawn_local(fut)
        });
    }

    fn vsync_events(&self) -> VsyncEvents { /* same shape as wasm */ }

    fn request_vsync(&self) {
        if !self.inner.vsync_requested.swap(true, AcqRel) {
            // JNI call: FrameLoop.scheduleVsync()
            self.jni_schedule_vsync();
        }
    }

    fn sleep(&self, d: Duration) -> Sleep {
        // tokio::time::sleep bridged via oneshot. tokio used for TIMERS ONLY.
        let runtime = self.runtime.clone();
        let (tx, rx) = futures::channel::oneshot::channel();
        runtime.spawn(async move {
            tokio::time::sleep(d).await;
            let _ = tx.send(());
        });
        Sleep(Box::pin(async move { let _ = rx.await; }))
    }
}

impl WorkerScheduler for AndroidSchedulerDriver {
    fn spawn_local<F: Future<Output = ()> + 'static>(&self, fut: F) {
        // Same thread-local dispatch as MainScheduler::spawn_local.
        CURRENT_POOL.with(|pool| {
            pool.borrow().as_ref().unwrap().spawner().spawn_local(fut)
        });
    }
    fn block_on<F, R>(&self, fut: F) -> R {
        CURRENT_POOL.with(|pool| pool.borrow_mut().as_ref().unwrap().run_until(fut))
    }
    fn sleep(&self, d: Duration) -> Sleep {
        // Same as MainScheduler::sleep (above).
        MainScheduler::sleep(self, d)
    }
}

impl AndroidInner {
    fn fire_vsync(&self) {
        self.vsync_requested.store(false, Release);
        if let Some(tx) = self.vsync_tx.borrow().as_ref() {
            let _ = tx.unbounded_send(());
        }
    }
}
```

`tur-android/Cargo.toml`: add `tokio = { workspace = true }`.

### `TestSchedulerDriver` (tur-integration-tests, new `test_scheduler.rs`)

```rust
pub struct TestSchedulerDriver {
    inner: Rc<TestInner>,
    main_pool: Rc<futures::executor::LocalPool>,
}
struct TestInner {
    vsync_tx: RefCell<Option<UnboundedSender<()>>>,
    vsync_requested: AtomicBool,
    // Virtual clock for Sleep
    now: Mutex<Instant>,
    timers: Arc<Mutex<BinaryHeap<Reverse<(Instant, u64)>>>>,
    timer_wakers: Arc<Mutex<HashMap<u64, Waker>>>,
    timer_signal: Arc<Condvar>,
    next_timer_id: AtomicU64,
    cancelled: Arc<Mutex<HashSet<u64>>>,
}

thread_local! {
    static CURRENT_POOL: RefCell<Option<Rc<LocalPool>>> = RefCell::new(None);
}

impl TestSchedulerDriver {
    pub fn new() -> Self {
        let driver = Self { inner: ..., main_pool: Rc::new(LocalPool::new()) };
        CURRENT_POOL.with(|c| *c.borrow_mut() = Some(driver.main_pool.clone()));
        // Spawn background timer thread that fires wakers at virtual deadlines.
        driver.spawn_timer_thread();
        driver
    }

    /// Fire one vsync event. The engine's autonomous loop processes it
    /// asynchronously on its worker thread.
    pub fn fire_vsync(&self) {
        self.inner.vsync_requested.store(false, Release);
        if let Some(tx) = self.inner.vsync_tx.borrow().as_ref() {
            let _ = tx.unbounded_send(());
        }
    }

    /// Block until the autonomous loop has processed the next FrameOutcome.
    /// Returns the outcome for inspection.
    pub fn step(&self) -> FrameOutcome { /* polls + waits */ }

    /// Keep firing vsyncs (as requested) until the engine goes Idle.
    pub fn run_until_idle(&self) { /* */ }

    /// Advance virtual clock by `d`, firing any pending Sleeps whose
    /// deadlines have been reached.
    pub fn advance(&self, d: Duration) {
        let mut now = self.inner.now.lock().unwrap();
        *now += d;
        self.inner.timer_signal.notify_all();
    }

    /// Drive an arbitrary future on the calling test thread.
    pub fn block_on<F, R>(&self, fut: F) -> R where F: Future<Output = R> {
        futures::executor::block_on(fut)
    }
}

impl MainScheduler for TestSchedulerDriver {
    fn spawn_worker<F, R>(&self, factory: F) -> WorkerHandle<R>
    where F: FnOnce() -> R + Send + 'static, R: Send + 'static
    {
        // Real std::thread (faithful to production threading).
        let join = std::thread::Builder::new()
            .name("tur-test-worker".into())
            .spawn(move || {
                let pool = Rc::new(futures::executor::LocalPool::new());
                CURRENT_POOL.with(|c| *c.borrow_mut() = Some(pool.clone()));
                let result = factory();
                CURRENT_POOL.with(|c| *c.borrow_mut() = None);
                result
            })
            .unwrap();
        WorkerHandle { join: Box::new(move || join.join().unwrap()) }
    }

    fn spawn_local<F: Future<Output = ()> + 'static>(&self, fut: F) {
        CURRENT_POOL.with(|pool| {
            pool.borrow().as_ref().unwrap().spawner().spawn_local(fut)
        });
    }
    fn vsync_events(&self) -> VsyncEvents { /* same shape */ }
    fn request_vsync(&self) {
        self.inner.vsync_requested.store(true, Release);
    }
    fn sleep(&self, d: Duration) -> Sleep { /* same as WorkerScheduler::sleep */ }
}

impl WorkerScheduler for TestSchedulerDriver {
    fn spawn_local<F: Future<Output = ()> + 'static>(&self, fut: F) {
        CURRENT_POOL.with(|pool| {
            pool.borrow().as_ref().unwrap().spawner().spawn_local(fut)
        });
    }
    fn block_on<F, R>(&self, fut: F) -> R {
        CURRENT_POOL.with(|pool| pool.borrow_mut().as_ref().unwrap().run_until(fut))
    }
    fn sleep(&self, d: Duration) -> Sleep {
        // Virtual clock + BinaryHeap + Condvar.
        let id = self.inner.next_timer_id.fetch_add(1, Relaxed);
        let deadline = *self.inner.now.lock().unwrap() + d;
        self.inner.timers.lock().unwrap().push(Reverse((deadline, id)));
        self.inner.timer_signal.notify_one();  // wake timer thread

        Sleep(Box::pin(poll_fn(move |cx| {
            if self.inner.cancelled.lock().contains(&id) { return Poll::Ready(()); }
            self.inner.timer_wakers.lock().insert(id, cx.waker().clone());
            Poll::Pending
        })))
    }
}

// Background timer thread per TestSchedulerDriver instance:
// loops on Condvar.wait_timeout(next_deadline - now), fires due wakers
// under virtual-clock semantics (wait_timeout returns immediately if
// now has been advanced past the deadline).
```

---

## 5. Runtime wiring

```rust
// TurRuntimeBuilder:
pub fn main_scheduler(mut self, sched: Rc<dyn MainScheduler>) -> Self {
    self.main_scheduler = Some(sched);
    self
}
pub fn worker_scheduler(mut self, sched: Rc<dyn WorkerScheduler>) -> Self {
    self.worker_scheduler = Some(sched);
    self
}

// Convenience: pass one driver that implements both traits.
pub fn scheduler<S>(self, driver: Rc<S>) -> Self
where S: MainScheduler + WorkerScheduler + 'static
{
    self.main_scheduler(driver.clone())
        .worker_scheduler(driver)
}

// TurRuntime stores both:
pub struct TurRuntime {
    main_scheduler: Rc<dyn MainScheduler>,
    worker_scheduler: Rc<dyn WorkerScheduler>,
    clock, font_context, font_loader, capabilities, plugins, ...
}

// create_app passes both to MainBackend + main to TurApp:
let backend = MainBackend::new(
    self.main_scheduler.clone(),
    self.worker_scheduler.clone(),
    backend_factory,
);
let app = Rc::new(TurApp::new(backend, self.main_scheduler.clone()));
```

**Why hold both separately:**
- Makes the dependency direction explicit at the type level (`MainScheduler` for main thread, `WorkerScheduler` for worker).
- Impossible to accidentally call `block_on` from main by typing the wrong field — the field is on a trait whose methods dispatch via thread-local, and the type system distinguishes the two surfaces.
- Allows the embedder to pass different impls (in theory) — e.g., a mock worker scheduler for testing one half. In practice, the convenience `.scheduler(driver)` method covers the common case where one driver impls both.

---

## 6. Embedder wiring

### tur-wasm (`WasmApp::create`)

```rust
let driver = Rc::new(WasmSchedulerDriver::new());
let runtime = TurRuntime::builder()
    .scheduler(driver.clone())   // sets both main + worker to the same driver
    .font_loader(Arc::new(WasmFontLoader::new()))
    .clock(Arc::new(WasmClock))
    /* plugins, capabilities */
    .build()?;
let app = runtime.create_app(viewport, dpr)?;
let app_clone = app.clone();
wasm_bindgen_futures::spawn_local(app_clone.start_loop());
```

### tur-android (`AndroidInstance::build_with_surface`)

```rust
// One shared tokio runtime per app.
let tokio_runtime = tokio::runtime::Runtime::new_multi_thread()?;
let driver = Rc::new(AndroidSchedulerDriver::new(
    tokio_runtime.handle().clone(),
    frame_loop,
));
let runtime = TurRuntime::builder()
    .scheduler(driver.clone())   // sets both main + worker to the same driver
    /* ... */
    .build()?;
let app = runtime.create_app(...)?;
let app_clone = app.clone();
std::thread::spawn(move || {
    futures::executor::block_on(app_clone.start_loop());
});
// Pass tokio_runtime.handle().clone() to HttpBackend (shared source)
```

### tur-integration-tests (test harness)

```rust
let driver = Rc::new(TestSchedulerDriver::new());
let runtime = TurRuntime::builder()
    .scheduler(driver.clone())   // sets both main + worker
    /* ... */
    .build()?;
let app = runtime.create_app((400.0, 600.0), 1.0)?;
let app_clone = app.clone();
let _worker = std::thread::spawn(move || {
    futures::executor::block_on(app_clone.start_loop());
});

// Drive frames + inspect state:
let outcome = driver.step();
assert!(outcome.rendered);
driver.run_until_idle();
driver.advance(Duration::from_millis(500));

// RPCs (still async) driven via test driver:
let tree = driver.block_on(app.dev_tool_element_tree())?;
```

---

## 7. Deletions

- `libs/tur-engine/src/core/thread.rs` (entire file)
- `libs/tur-engine/src/core/async_/executor.rs::AsyncExecutor` (TurJobExecutor stays — different concern, it's boa's PromiseJob queue)
- `libs/tur-engine/src/core/async_/task.rs::sleep` (replaced by `SchedulerDriver::sleep`)
- `libs/tur-engine/src/lib.rs::LoopDriver` trait, `WakeFuture`, `SpawnWake`
- `libs/tur-engine/src/lib.rs::TurApp::start`, `TurApp::spawn_wake`, `TurApp::wake`, `TurApp::pump` (replaced by `start_loop` + `handle_main_msg`)
- All `pump_in_progress` / `wake_pending` / `in_flight` / `WakeNotify` state on TurApp
- `NextFrame::After(d)` variant + cascade through `flush()`, `Shell`, all match arms
- `Shell::next_timer_delay` clock-advance math
- `libs/tur-engine/Cargo.toml`: drop `wasm_thread` (target-gated), drop `tur-async`
- `libs/tur-android/src/loop_driver.rs` (replaced by `scheduler.rs`)
- `libs/tur-wasm/src/app.rs::WasmLoopDriver` (replaced by `WasmSchedulerDriver`)
- `libs/tur-native/` (entire crate — empty)

---

## 8. Cargo.toml changes

- **tur-engine**: drop `wasm_thread` (target-gated), drop `tur-async`
- **tur-wasm**: add `wasm_thread = { workspace = true }`
- **tur-android**: add `tokio = { workspace = true }`
- **Delete** `libs/tur-native/Cargo.toml` + `libs/tur-native/src/lib.rs`

---

## 9. Execution order on branch

1. Add new `core/scheduler.rs` module + `Sleep` + `VsyncEvents` + `WorkerSchedulerView` + `WorkerHandle` types. Compiles standalone.
2. Add `core/async_/completion.rs` (`CompletionQueue` + `CompletionHandle`). Compiles standalone.
3. Implement `WasmSchedulerDriver` in tur-wasm (port rAF/setTimeout logic from WasmLoopDriver; hand-rolled Sleep).
4. Implement `AndroidSchedulerDriver` in tur-android (port JNI logic; tokio sleep via Handle; std::thread for spawn_worker).
5. Implement `TestSchedulerDriver` in tur-integration-tests (real threads + virtual clock + Condvar + BinaryHeap + helper methods `step()` / `run_until_idle()` / `advance()`).
6. Rewrite `MainBackend::new` to take `Rc<dyn SchedulerDriver>` + use `spawn_worker`. Drop `core::thread` import + native init-signal dance.
7. Rewrite `TurApp` — drop LoopDriver/spawn/pump fields + methods; add `start_loop`, `handle_main_msg`. Drop `in_flight`/`wake_pending`/`pump_in_progress`.
8. Rewrite `WorkerBackend` to capture `worker_tx` for `CompletionHandle::on_push` self-wake.
9. Rewrite bridges (clipboard/http/filepicker/caret-blink) — use `worker_view.spawn_local` + `completion_handle.push`.
10. Drop `NextFrame::After(d)` variant + cascade through `flush()`, `Shell`, all match arms.
11. Drop `AsyncExecutor` + `core/async_/task.rs::sleep` + `tur-async` dep.
12. Drop `LoopDriver` + `WakeFuture` + `SpawnWake` + `TurApp::pump` from `lib.rs`.
13. Drop `core::thread` module + `wasm_thread` dep from `tur-engine/Cargo.toml`.
14. Update cdylibs (`demo/website/native`, `demo/compose/native`) to construct the driver + spawn `start_loop`.
15. Rewrite integration tests to use `start_loop` + `TestSchedulerDriver::step()`.
16. Delete `libs/tur-native/` crate.
17. Verification:
    - `cargo test --workspace`
    - `cargo clippy --workspace -- -D warnings`
    - `cd demo/website/native && wasm-pack build --target web --no-opt`
    - Dev server visual check (`cd demo/website && pnpm dev`)

---

## 10. Risks

- **Test rewrite is the largest single piece** (~100 files). Mitigation: rich `TestSchedulerDriver::step()` helper makes most tests change only a few lines (replace `block_on(app.pump())` with `driver.step()`).
- **Bridge migration** is high-risk — easy to miss a `spawn_detached` callsite. Mitigation: full test suite + clippy catch most; manual visual check on dev server.
- **`TestSchedulerDriver` virtual clock + Condvar + real threads** is the most intricate piece (~150 lines). Cancellation, race conditions, deterministic firing order need care.
- **Threading change in tests**: tests now spawn real worker threads. Concurrent test execution may stress thread count. Cargo test parallelism may need tuning (`--test-threads=N`).
- **Performance**: this PR is structural. The rAF-churn perf bug is fixed as a side effect (idempotent `request_vsync`); other perf characteristics should be unchanged.

---

## 11. Out of scope (explicitly deferred)

- Reducing per-frame render-command shipping overhead
- Any rearchitecture of `MainBackend`'s channel topology
- Desktop native target (no plan; `AndroidSchedulerDriver` is android-only)

---

## 12. Architectural properties

- **Dependency direction**: engine → scheduler, one-way. Driver has zero engine knowledge.
- **Two-trait split**: `MainScheduler` (main-only methods) and `WorkerScheduler` (worker-only methods). Same driver object implements both; runtime holds both trait objects. Thread-locals inside the impl dispatch to the right per-thread state.
- **Main ≠ worker thread**: enforced by `std::thread::spawn` (native) or Web Worker (wasm) in `spawn_worker`.
- **Event-driven**: single event stream loop in `start_loop`, no `set_wake` callback registration.
- **Idempotent vsync**: `request_vsync` is a no-op if already armed — kills rAF churn.
- **No engine-side timer queue**: `Sleep` is platform-driven; wakes worker via `CompletionHandle::on_push` → self-Wake.
- **Engine is tokio-free**: tokio dep lives in the native driver (tur-android) only.
- **Engine is wasm_thread-free**: dep moves to tur-wasm.
