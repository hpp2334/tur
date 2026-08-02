# Phase 7 — Multi-threaded runtime on native (worker pool, off-main-thread)

**Status:** 🚧 partial — prerequisite landed (Plugin `Send + Sync` + test plugin migration); worker thread + TurApp restructure pending
**Prerequisite:** Phase 4 (`WorkerMsg`/`MainMsg` wire format + `handle_worker_msg` dispatch).
**Goal:** introduce `TurRuntimeBuilder::tokio_handle(handle)` + a `WorkerPool` (Model B: pool of `min(4, num_cpus)` workers, lazy-spawned via `handle.spawn_blocking`, round-robin instance assignment at `create_app`). The boa Context + tree + subsystems for each instance live on its assigned worker thread; the renderer + cursor backend stay on main. **Native (desktop + Android) now runs JS off the main thread.**

## ✅ What landed (prerequisite)

- `Plugin: Send + Sync` supertrait added. All six production plugins are zero-field unit structs — trivially satisfied.
- `HostModulePlugin` (test util) refactored from holding pre-built `NativeFunction`s (which wrap boa `TraceableClosure`, `!Send`) to holding **builder closures** (`Box<dyn Fn(&mut Context) -> NativeFunction + Send + Sync>`). Each instance's `register()` calls the builder to produce a fresh `NativeFunction` against its own boa `Context`.
- `CapturePlugin` / `CounterPlugin` (test plugins) migrated from `Rc<RefCell<>>` / `Rc<Cell<>>` to `Arc<Mutex<>>` / `Arc<AtomicU32>`. Test assertions updated (pointer-equality replaced with `Arc::ptr_eq` / value comparison to avoid deadlock when comparing the same Mutex twice).
- 2 test files updated: `host_module_check.rs`, `reentrant_module_check.rs` use the new `HostExport { name, builder, length }` shape.
- 2 test files updated: `capability.rs`, `multi_instance.rs` use `Arc`/`Mutex`/`AtomicU32` instead of `Rc`/`RefCell`/`Cell` for plugin-shared state.
- **`instance_data` removed entirely** (per user direction): `EventBus` is the only consumer, so the type-erased `HashMap<TypeId, Box<dyn Any>>` on `TurAppInternal` was overkill.
  - `EventBusInner` merged into `EventBus` (state lives directly on `EventBus`, shared via `Rc<EventBus>` — no separate "inner" type).
  - `EventBus` + bridge closures + `HostBusSubsystem` + `install_event_bus` moved to new `core::event_bus` module (engine infrastructure, not plugin-specific).
  - `TurAppInternal.event_bus: Rc<EventBus>` constructed up-front in `new()` (no longer populated during plugin register).
  - `PluginContext.event_bus: Rc<EventBus>` field + `event_bus()` accessor — replaces `store_instance_data<T>`.
  - `TurApp::event_bus()` returns `Rc<EventBus>` directly (no `from_inner` wrapper).
  - `TurApp::instance_data<T>()` pub method removed.
  - `builtin_plugins/event_bus/` deleted (was redundant after move).
  - 170 element + 93 event tests pass; wasm + clippy clean.

## 🚧 What remains

### 7.1 — `TurApp` restructure (the big rock)

Today's `TurApp` owns `boa_context: RefCell<Context>` + `internal: TurAppInternal` + `executor: Rc<TurJobExecutor>` inline. These are all `!Send` (boa GC + `Rc` graph rooted in the `Context`).

For multi-threading, two architectural options:

**A. TurAppBackend trait object (cleanest, verbose)**
- Introduce `trait TurAppBackend` with `InlineBackend` (today's state) and `ThreadedBackend` (worker thread + channels).
- All `TurApp` methods delegate to the backend.
- Existing tests use InlineBackend (default); production uses ThreadedBackend via `create_app_threaded`.
- The public `TurApp` type stays unchanged — embedders see no API break.
- ~600 LOC new (trait + InlineBackend move + ThreadedBackend + worker thread loop), ~300 LOC modified.

**B. Single threaded TurApp (what the original plan called for)**
- Move all engine state to a worker thread unconditionally.
- Every `TurApp` method goes through channels.
- Escape hatches (`with_element`, `with_boa_context`, `with_app`) become synchronous RPC (send `WorkerMsg`, blocking_recv on reply).
- ~40 test files migrate to the new pattern.
- ~1000 LOC total, high regression risk.

### 7.2 — Escape hatch RPC variants

Either approach needs `WorkerMsg::WithElement { id, closure, reply }` and `WorkerMsg::WithBoaContext { closure, reply }` for the synchronous test-introspection pattern. Closures must be `Send + Sync` (they cross thread boundaries). Return types must be `Send`. ~40 test files use this pattern; each call site picks up the RPC overhead.

### 7.3 — Per-worker capability factory pattern

Per the "local thread shared" decision (Phase 6 cancelled): each worker thread constructs its own backends via `Send + Sync` factory closures stored in `RuntimeConfig`. The runtime hands factories to the worker; the worker builds its `Rc`-rooted capability map on first spawn.

### 7.4 — Worker thread loop

```rust
fn worker_loop(
    worker_rx: mpsc::Receiver<WorkerMsg>,
    main_tx: mpsc::Sender<MainMsg>,
    config: Arc<RuntimeConfig>,
) {
    // Build per-worker state (capabilities from factories, plugins, etc.).
    let local_caps = build_local_capabilities(&config.capability_factories);

    // Per-instance state built on demand when CreateInstance arrives.
    // For Model A (one worker per instance), this is one InstanceState.
    let mut instances: HashMap<InstanceId, InstanceState> = HashMap::new();

    for msg in worker_rx.iter() {
        match msg {
            WorkerMsg::Wake => {
                // Run flush on the assigned instance. Worker owns the
                // renderer, so flush() renders directly. FrameOutcome
                // is shipped to main.
                let outcome = instance.flush();
                let _ = main_tx.send(MainMsg::FrameOutcome(outcome));
            }
            // ... other variants delegate to per-instance dispatch.
            WorkerMsg::Destroy { .. } => break,
            _ => instance.handle_worker_msg(msg),
        }
    }
}
```

### 7.5 — `NativeHttp::new(handle)` unchanged

The "local thread shared" model (Phase 6 cancelled) means each worker constructs its own `NativeHttp` from a cloned `Handle`. The runtime stores the `Handle` (or a factory); each worker calls `NativeHttp::new(handle.clone())` at startup.

## Why this is genuinely big

Refactoring `TurApp` is a deep surgery:
- `boa_engine::Context` is `!Send` (GC-thread-local).
- `Rc<TurAppInternal>` is `!Send`.
- The whole engine graph (`ElementTree`, `FocusManager`, `MutationQueue`, `ImageResourceMap`, `Store`, `AsyncExecutor`, `Capabilities`) is `Rc`-rooted.
- Moving any of it across threads requires either Arc/Mutex throughout OR a worker thread that owns it all.

The "local thread shared" decision keeps backends on `Rc` (per-worker), but `TurAppInternal` itself is `Rc`-rooted and would need the same per-worker treatment OR a worker that owns it.

## Risks (unchanged from original plan)

- **boa Context construction time** — building a fresh Context per instance on the worker is currently done synchronously in `create_app`. With the split, it happens on the worker asynchronously; the first `pump()` may stall. Mitigation: construct eagerly in `create_app` (block on the spawn_blocking join) OR show a loading state.
- **Channel overhead** — every event and every frame's commands cross a channel. With `min(4, num_cpus)` workers, contention is minimal. mpsc is fast.
- **Cursor latency** — pointer events arrive at main, forward to worker, worker resolves cursor claim during record, ships back. +1 frame inherent latency.
- **Event bus handler timing** — handlers now run on main (during pump), not inline during flush. Slight timing shift; document.
- **Per-worker factory pattern is new code.** Backends must be constructible from a `Send + Sync` factory closure. For `Rc`-only backends (none today, but a future macOS-specific arboard variant could need this), the factory must produce fresh instances without sharing parent-thread state.

## Out of scope

- Wasm threading — Phase 9.
- Android-specific JNI changes — Phase 8.
- Public API stable; only the runtime builder requires the new `tokio_handle` arg.

## Estimated scope (remaining)

- ~700 lines new (TurAppBackend trait + InlineBackend + ThreadedBackend + worker thread loop + per-worker factory plumbing)
- ~300 lines modified (TurApp methods, TurRuntimeBuilder, create_app)
- ~200 lines test migration (escape hatch RPC pattern)
- Significant PR; budget 4-6 days including multi-instance testing + factory pattern design
