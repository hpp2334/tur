# Phase 7 — Multi-threaded runtime on native (worker pool, off-main-thread)

**Status:** not started
**Prerequisite:** Phase 4 (`WorkerMsg`/`MainMsg` wire format + `handle_worker_msg` dispatch).
**Goal:** introduce `TurRuntimeBuilder::tokio_handle(handle)` + a `WorkerPool` (Model B: pool of `min(4, num_cpus)` workers, lazy-spawned via `handle.spawn_blocking`, round-robin instance assignment at `create_app`). The boa Context + tree + subsystems for each instance live on its assigned worker thread; the renderer + cursor backend stay on main. **Native (desktop + Android) now runs JS off the main thread.**

## Background

boa `Context` is `!Send` (GC-thread-local). Each instance's JS state is bound to one OS thread for the instance's lifetime. Model B (worker pool with instance pinning) lets M threads serve N instances — each instance permanently assigned to one worker, the worker multiplexes its instances cooperatively.

The user-provided tokio runtime owns the worker pool. Engine never constructs a runtime; it submits long-running worker tasks via `handle.spawn_blocking`. Communication is `tokio::sync::mpsc` / `oneshot`.

### Per-worker "local thread shared" capability model

Each worker thread constructs its **own** `Rc`-rooted backend graph from a `RuntimeConfig` of factories/closures. Backends stay `Rc<dyn Backend>` (no `Arc`, no `Send + Sync` bounds on the trait). Backends are shared **within** a worker (across instances pinned to that worker); **not shared across** worker threads. This replaces the original Phase 6 plan's "shared Arc-backed backends" approach — most backends are stateless dispatchers (WasmClipboard → `navigator.clipboard`, WasmHttp → fresh reqwest client per request, etc.) and have no cross-instance state worth sharing across OS threads.

`TurRuntime` therefore stores **backend factories** (closures producing a fresh `Rc<dyn Backend>` per call), not backend instances. When a worker thread first spins up, it calls each factory to build its own backend set, then builds a `Capabilities` map from those.

## Architecture

```
┌────────────── EMBEDDER ──────────────┐
│  tokio::runtime::Runtime (multi-thr) │
│  ├── async worker pool (HTTP, etc.)  │
│  └── blocking pool (worker tasks)    │
└──────────────┬───────────────────────┘
               │ Handle
               ▼
┌─────────── TurRuntime ───────────────┐
│  plugins: Vec<Box<dyn Plugin>>       │ ← shared by reference; per-worker
│  capability_factories: Factories     │   calls each to build local set
│  font_context: FontContext           │ ← Arc-backed internally, shareable
│  clock: Arc<dyn Clock>               │ ← shareable
│  font_loader: Arc<dyn FontLoader>    │ ← shareable
│  tokio_handle: Handle                │
│  worker_pool: Arc<WorkerPool>        │
└──────────────────────────────────────┘

WorkerPool:
  - workers: Vec<mpsc::Sender<WorkerCommand>>
  - next_assignment: AtomicUsize  (round-robin)
  - pool_size = min(4, num_cpus)
  - Lazy spawn: workers vec starts empty; first instance to slot i
    triggers handle.spawn_blocking(worker_loop)

worker_loop (one OS thread per pool slot):
  - rx: mpsc::Receiver<WorkerCommand>
  - instances: HashMap<InstanceId, InstanceState>
  - local_caps: Capabilities  ← built fresh on THIS thread from factories
  - loop:
      recv WorkerCommand:
        CreateInstance { config, reply }:
          build InstanceState (boa Context + tree + subsystems + ...)
            — uses local_caps (Rc clones, same thread)
          insert into instances map
          reply.send(main_rx_for_this_instance)
        InstanceMessage { id, msg }:
          process on instances[id]
          (drain WorkerMsg queue → flush → build MainMsgs → send via instance's main_tx)
        DestroyInstance { id }:
          run before_destroy hooks; drop state
```

## Files to add

### `libs/tur-engine/src/core/runtime/worker_pool.rs`

```rust
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::runtime::Handle;

use crate::core::app::comm::{WorkerMsg, MainMsg};
use crate::core::capability::Capabilities;
use crate::core::element::ElementNodeId;
use crate::core::plugin::Plugin;

pub struct WorkerPool {
    workers: Vec<mpsc::Sender<WorkerCommand>>,
    next_assignment: AtomicUsize,
    handle: Handle,
    pool_size: usize,
    /// Shared, Send+Sync config (factories + Arc-backed resources).
    runtime_config: Arc<RuntimeConfig>,
}

/// Send + Sync runtime-level config. Each worker calls
/// `runtime_config.build_local_state()` on its own thread to construct a
/// fresh `Rc`-rooted backend graph + plugin set.
pub struct RuntimeConfig {
    pub plugin_factories: Vec<Box<dyn Fn() -> Box<dyn Plugin> + Send + Sync>>,
    pub capability_factories: Vec<CapabilityFactory>,
    pub font_context: FontContext,
    pub clock: Arc<dyn Clock>,
    pub font_loader: Arc<dyn FontLoader>,
}

/// A typed closure that produces a fresh capability value on the worker
/// thread. Each backend (Clipboard/Http/FilePicker) is registered here as
/// a factory; the worker builds its own `Rc<dyn Backend>` instances.
pub struct CapabilityFactory {
    pub type_id: std::any::TypeId,
    pub type_name: &'static str,
    pub build: Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync>,
}

enum WorkerCommand {
    CreateInstance {
        config: InstanceConfig,
        reply: oneshot::Sender<Result<InstanceHandle, SpawnError>>,
    },
    DestroyAll {
        reply: oneshot::Sender<()>,
    },
}

pub struct InstanceConfig {
    pub viewport: (f64, f64),
    pub dpr: f64,
}

pub struct InstanceHandle {
    pub worker_tx: mpsc::Sender<WorkerMsg>,
    pub main_rx: mpsc::Receiver<MainMsg>,
    pub instance_id: InstanceId,
}

impl WorkerPool {
    pub fn new(handle: Handle, config: RuntimeConfig) -> Self {
        let pool_size = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(4);
        WorkerPool {
            workers: Vec::with_capacity(pool_size),
            next_assignment: AtomicUsize::new(0),
            handle,
            pool_size,
            runtime_config: Arc::new(config),
        }
    }

    pub async fn create_instance(&mut self, config: InstanceConfig) -> Result<InstanceHandle, SpawnError> {
        let slot = self.next_assignment.fetch_add(1, Ordering::Relaxed) % self.pool_size;
        if self.workers.len() <= slot {
            self.spawn_worker(slot);
        }
        let (tx, rx) = oneshot::channel();
        self.workers[slot].send(WorkerCommand::CreateInstance { config, reply: tx }).await?;
        rx.await?
    }
}

fn worker_loop(
    mut rx: mpsc::Receiver<WorkerCommand>,
    config: Arc<RuntimeConfig>,
) {
    // Build this worker's local state — Rc-rooted, never crosses threads.
    let local_caps = build_local_capabilities(&config.capability_factories);
    let local_plugins: Vec<Box<dyn Plugin>> =
        config.plugin_factories.iter().map(|f| f()).collect();
    let mut instances: HashMap<InstanceId, InstanceState> = HashMap::new();

    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            WorkerCommand::CreateInstance { config: ic, reply } => {
                let state = InstanceState::build(&config, &local_caps, &local_plugins, ic);
                let handle = InstanceHandle { /* ... */ };
                instances.insert(state.id, state);
                let _ = reply.send(Ok(handle));
            }
            WorkerCommand::DestroyAll { reply } => {
                for (_, mut s) in instances.drain() {
                    s.shutdown();
                }
                let _ = reply.send(());
                return;
            }
        }
    }
}

fn build_local_capabilities(factories: &[CapabilityFactory]) -> Capabilities {
    let caps = Capabilities::new();
    for f in factories {
        let value = (f.build)();
        // Insert via the typed API. The closure returns Box<dyn Any>;
        // Capabilities::insert expects a typed C: Capability, so we need
        // a typed-erased insertion path here. (Implementation detail —
        // likely a `Capabilities::insert_erased(TypeId, Box<dyn Any>)`
        // added for this purpose.)
        caps.insert_erased(f.type_id, value);
    }
    caps
}
```

### `libs/tur-engine/src/core/runtime/instance_state.rs`

The worker-side `!Send` state for one instance. Same shape as today's `TurAppInternal` + boa `Context` + comm channels. Built on the worker thread (boa `Context` constructed here — never crosses thread boundary).

```rust
pub struct InstanceState {
    pub id: InstanceId,
    pub boa_context: Context,  // !Send
    pub internal: TurAppInternal,
    pub executor: Rc<TurJobExecutor>,
    pub main_tx: mpsc::Sender<MainMsg>,
    pub worker_rx: mpsc::Receiver<WorkerMsg>,
}

impl InstanceState {
    pub fn build(
        config: &RuntimeConfig,
        local_caps: &Capabilities,
        local_plugins: &[Box<dyn Plugin>],
        instance: InstanceConfig,
    ) -> Self { ... }

    /// Process one message from main. Drains event queues, runs flush,
    /// builds MainMsgs, sends them via `main_tx`.
    pub fn process(&mut self, msg: WorkerMsg) { ... }

    pub fn shutdown(&mut self) { ... }
}
```

## Files to modify

### `libs/tur-engine/src/core/runtime.rs` — `TurRuntimeBuilder`

Add `tokio_handle` + factory registration. Today's `.capability(Clipboard::new(backend))` becomes `.capability_factory(Clipboard::factory(backend))` (or the existing API is kept and the builder wraps the backend in a closure that clones it — but `Rc` is `!Clone`-across-threads, so the cleanest is a factory-style API).

```rust
pub struct TurRuntimeBuilder {
    font_loader: Option<Rc<dyn FontLoader>>,
    clock: Option<Rc<dyn Clock>>,
    capability_factories: Vec<CapabilityFactory>,
    plugins: Vec<Box<dyn Plugin>>,
    tokio_handle: Option<Handle>,
}

impl TurRuntimeBuilder {
    pub fn tokio_handle(mut self, handle: Handle) -> Self {
        self.tokio_handle = Some(handle);
        self
    }
    pub fn build(self) -> Result<Rc<TurRuntime>, TurError> {
        let handle = self.tokio_handle.ok_or_else(|| TurError::other(
            "tokio Handle is required (TurRuntimeBuilder::tokio_handle)"
        ))?;
        // ... build WorkerPool, RuntimeConfig (factories + Arc-backed resources)
    }
}
```

### `TurRuntime::create_app`

```rust
pub fn create_app(
    self: &Rc<Self>,
    renderer: Box<dyn Renderer>,
    viewport: (f64, f64),
    dpr: f64,
) -> Result<Rc<TurApp>, TurError> {
    // Submit CreateInstance to the worker pool (round-robin slot).
    let handle = self.worker_pool.blocking_create_instance(InstanceConfig {
        viewport, dpr,
    })?;
    Ok(Rc::new(TurApp::new(handle, renderer)))
}
```

`TurApp::new` constructs the main-side handle with the worker channels.

### `TurApp` becomes the main-side handle

```rust
pub struct TurApp {
    worker_tx: mpsc::Sender<WorkerMsg>,
    main_rx: RefCell<mpsc::Receiver<MainMsg>>,
    renderer: RefCell<Box<dyn Renderer>>,
    cursor_backend: RefCell<Option<Rc<RefCell<dyn CursorBackend>>>>,
    focused_state: RefCell<FocusedState>,
    last_cursor: RefCell<Option<Cursor>>,
    main_tree: RefCell<MainTree>,
    event_bus: EventBus,  // always-installed, hand-built on main
    viewport: RefCell<(u32, u32, f64)>,
}
```

Per-frame pump (called by embedder's rAF):
```rust
impl TurApp {
    pub fn pump(&self) -> Result<FrameOutcome, TurError> {
        // 1. Forward any queued WorkerMsg (the embedder calls push_platform_event etc.)
        // 2. Send Wake.
        // 3. Drain main_rx until FrameOutcome.
        // 4. Return FrameOutcome for the embedder to schedule next rAF.
    }
}
```

### Embedder rAF loop (e.g. `tur-wasm`, `tur-android`)

Today's `run_frame()` becomes `pump()`:
```rust
let outcome = app.pump()?;
driver.request_next(outcome.schedule);
```

## Verification

1. **Multi-instance integration test** — `tests/element/multi_instance.rs` extended: spawn 5+ instances, drive each independently, verify isolation. With pool_size = min(4, num_cpus), instances share workers.
2. **Off-main-thread verification** — log the thread ID inside `flush()` and inside `Renderer::render_commands`. They should differ.
3. All existing element + event + vello tests pass (they go through the same message path, just inlined on the test thread — pump uses `try_recv`/`blocking_recv`).
4. **No blocking on main** — main never waits synchronously for a worker round-trip during normal flush. Only the *first* `pump()` after `create_app` may take longer (initial instance construction on the worker).
5. `cargo test --workspace` clean.
6. `cargo clippy --workspace -- -D warnings` clean.
7. Manual smoke test: playground + Android demo render correctly.

## Risks

- **boa Context construction time** — building a fresh Context per instance on the worker is currently done synchronously in `create_app`. With the split, it happens on the worker asynchronously; the first `pump()` may stall. Mitigation: construct eagerly in `create_app` (block on the spawn_blocking join) OR show a loading state.
- **Channel overhead** — every event and every frame's commands cross a channel. With `min(4, num_cpus)` workers, contention is minimal. mpsc is fast.
- **Cursor latency** — pointer events arrive at main, forward to worker, worker resolves cursor claim during record, ships back. +1 frame inherent latency. Acceptable per earlier design.
- **Event bus handler timing** — handlers now run on main (during pump), not inline during flush. Slight timing shift; document.
- **Per-worker factory pattern is new code.** Backends must be constructible from a `Send + Sync` factory closure. For `Rc`-only backends (none today, but a future macOS-specific arboard variant could need this), the factory must produce fresh instances without sharing parent-thread state.

## Out of scope

- Wasm threading — Phase 9.
- Android-specific JNI changes — Phase 8.
- Public API stable; only the runtime builder requires the new `tokio_handle` arg.

## Estimated scope

- ~700 lines new (WorkerPool, InstanceState, per-worker factory plumbing, comm wiring)
- ~300 lines modified (TurRuntimeBuilder, create_app, TurApp)
- Significant PR; budget 4-6 days including multi-instance testing + factory pattern design
