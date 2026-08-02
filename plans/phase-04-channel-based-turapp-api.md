# Phase 4 — Channel-based `TurApp` API (single-threaded)

**Status:** ✅ complete (refined scope — see "Outcome" below)
**Prerequisite:** Phase 3 (commands are the render path).
**Goal:** define the worker↔main wire format (`WorkerMsg` / `MainMsg`) and route every existing pub fn through a single `TurApp::handle_worker_msg` dispatch. Still single-threaded in this phase; the dispatch is an inline call. **Phase 7 will swap the inline call for a real `mpsc` send + worker task without changing the wire types or the public method shapes.**

## Background

Today's `TurApp` pub fn directly mutates shared `Rc<RefCell<>>` state. Once we split threads in Phase 7, that's impossible — main can't borrow worker state. Phase 4 introduces the message-based API surface so the thread split in Phase 7 is purely a "wire the channels across threads" change, not a "redesign every API" change.

The messages live in `core/render/comm.rs` (or `core/app/comm.rs`).

## Files to add

### `libs/tur-engine/src/core/app/comm.rs`

```rust
use std::sync::Arc;
use tokio::sync::oneshot;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::platform::PlatformEvent;
use crate::core::app::event::AppEvent;
use crate::core::platform::Cursor;
use crate::core::render::RenderCommand;
use crate::core::app::FrameOutcome;
use crate::dev::DevNodeData;

/// main → worker. All cross-thread input flows through this.
#[derive(Debug)]
pub enum WorkerMsg {
    /// DOM/JNI/winit platform event (pointer, key, wheel, IME, resize, ...).
    PlatformEvent(PlatformEvent),
    /// Engine-internal event (scroll-to, clipboard-paste, ...).
    AppEvent(AppEvent),
    /// Mark the frame dirty without a specific event (e.g. the embedder
    /// observed an external state change).
    RequestPaint,
    /// Drive one flush iteration. Sent by main's rAF loop.
    Wake,
    /// Parse + evaluate a JS module. Reply via `MainMsg::ModuleReady`.
    /// `Arc<str>` because module sources can be large (the playground
    /// ships multi-KB compiled JS).
    LoadModule { source: Arc<str>, reply: oneshot::Sender<Result<(), ModuleError>> },
    LoadJs { source: Arc<str>, reply: oneshot::Sender<Result<(), ModuleError>> },
    EvalModule { source: Arc<str>, reply: oneshot::Sender<Result<(), ModuleError>> },
    /// Dev-tool queries (async RPC).
    DevElementTree { reply: oneshot::Sender<DevNodeData> },
    DevGetElement { id: NodeId, reply: oneshot::Sender<Option<DevNodeData>> },
    /// Event bus — host → JS bytes.
    EventBusToJs(Vec<u8>),
    /// Initiate shutdown. Worker drains pending work, replies when safe to drop.
    Destroy { reply: oneshot::Sender<()> },
}

/// worker → main.
#[derive(Debug)]
pub enum MainMsg {
    /// One frame's worth of paint state. Main applies + renders.
    RenderCommands(Vec<RenderCommand>),
    /// Schedule decision after a flush. Main arms the next rAF / setTimeout.
    FrameOutcome(FrameOutcome),
    /// Resolved cursor changed this frame (deduped: only emitted on change).
    CursorChanged(Cursor),
    /// Focused-element state changed (for IME / caret placement on main).
    FocusedStateChanged {
        is_editable: bool,
        cursor_rect: Option<(f64, f64, f64, f64)>,
    },
    /// Event bus — JS → host bytes. Emitted per-message.
    EventBusToHost(Vec<u8>),
    /// Reply to a `WorkerMsg::LoadModule` / `LoadJs` / `EvalModule`.
    ModuleReady(Result<(), ModuleError>),
    /// Reply to dev-tool queries.
    DevReply(DevReply),
    /// Worker is shutting down (response to `WorkerMsg::Destroy`).
    Destroyed,
}

#[derive(Debug)]
pub enum DevReply {
    ElementTree(DevNodeData),
    GetElement(Option<DevNodeData>),
}

#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    #[error("JS parse error: {0}")]
    Parse(String),
    #[error("JS evaluation error: {0}")]
    Eval(String),
    #[error("worker gone")]
    WorkerGone,
}
```

## Files to modify

### `libs/tur-engine/src/lib.rs` — `TurApp` becomes a handle

```rust
pub struct TurApp {
    /// Channel to the worker (or in single-threaded mode, to ourselves).
    worker_tx: tokio::sync::mpsc::UnboundedSender<WorkerMsg>,
    /// Channel from the worker. Main pumps this each rAF.
    main_rx: tokio::sync::mpsc::UnboundedReceiver<MainMsg>,
    /// Renderer (main-side only).
    renderer: RefCell<Box<dyn Renderer>>,
    /// Cursor backend (main-side only).
    cursor_backend: RefCell<Option<Rc<RefCell<dyn CursorBackend>>>>,
    /// Cached hot-path state, updated by `MainMsg::FocusedStateChanged` /
    /// `CursorChanged`. Main reads these directly — no RPC.
    focused_state: RefCell<FocusedState>,
    last_cursor: RefCell<Option<Cursor>>,
    /// Main-side render tree (kept for dev tools / future main-side queries).
    main_tree: RefCell<MainTree>,
    /// Worker runtime config (used during construction).
    internal: TurAppInternal,  // behind a cfg for single-threaded mode
    ...
}
```

Public API migration (today → tomorrow):

| Today | Tomorrow |
|---|---|
| `push_platform_event(ev)` | `worker_tx.send(WorkerMsg::PlatformEvent(ev))` |
| `push_app_event(ev)` | `worker_tx.send(WorkerMsg::AppEvent(ev))` |
| `request_paint()` | `worker_tx.send(WorkerMsg::RequestPaint)` |
| `run_frame()` | Removed. Main sends `Wake`, drains `main_rx` until `FrameOutcome`. |
| `load_module(src)` (sync) | `async fn load_and_run_module(Arc<str>) -> Result<...>` — sends LoadModule, awaits oneshot |
| `load_js(src)` | Same pattern |
| `eval_module(src)` | Same pattern |
| `dev_tool_element_tree()` (sync) | `async fn dev_tool_element_tree() -> DevNodeData` |
| `focused_is_editable()` | Reads cached `focused_state.is_editable` (no RPC) |
| `focused_cursor_rect()` | Reads cached `focused_state.cursor_rect` (no RPC) |
| `set_after_frame_hook(hook)` | Removed. Main consumes `MainMsg::FrameOutcome` and runs its own logic. |
| `set_cursor_backend(b)` | Stays — main-side only. |
| `start(driver)` | Engine internal. Worker auto-starts on construction. |

### `libs/tur-engine/src/core/runtime.rs` — `create_app`

Today builds the boa Context + tree + subsystems directly. Tomorrow: constructs the worker task (in single-threaded Phase 4 mode, the "worker" runs inline; in Phase 7 it's `handle.spawn_blocking`).

### Main loop (per embedder)

Each rAF tick on main:
```rust
// 1. Forward any DOM/JNI events collected since last frame.
while let Some(ev) = embedder_events.drain() {
    app.worker_tx.send(WorkerMsg::PlatformEvent(ev)).ok();
}

// 2. Tell the worker to pump.
app.worker_tx.send(WorkerMsg::Wake).ok();

// 3. Drain worker replies until we get the FrameOutcome for this frame.
loop {
    let Some(msg) = app.main_rx.blocking_recv() else { break };
    match msg {
        MainMsg::RenderCommands(batch) => {
            app.main_tree.borrow_mut().apply_batch(batch);
            // Render from the batch (Option B from Phase 3).
        }
        MainMsg::CursorChanged(c) => {
            if let Some(backend) = app.cursor_backend.borrow().as_ref() {
                backend.borrow_mut().set_cursor(c);
            }
        }
        MainMsg::FocusedStateChanged { is_editable, cursor_rect } => {
            app.focused_state.borrow_mut().update(is_editable, cursor_rect);
        }
        MainMsg::FrameOutcome(outcome) => {
            // Arm next rAF based on outcome.schedule.
            driver.request_next(outcome.schedule);
            break;
        }
        MainMsg::EventBusToHost(bytes) => {
            event_bus_handle.dispatch(bytes);
        }
        MainMsg::ModuleReady(res) => { /* fires a oneshot already */ }
        MainMsg::DevReply(r) => { /* fires a oneshot already */ }
        MainMsg::Destroyed => break,
    }
}
```

Single-threaded mode: `blocking_recv` works because nothing else is happening on the thread. Multi-threaded (Phase 7): main uses `try_recv` and rAFs at 60Hz, painting whatever's latest.

## Files to migrate

- `tur-wasm/src/app.rs` — `WasmApp::create` constructs via new API; rAF loop drains `main_rx`.
- `tur-android/src/app.rs` — `AndroidInstance::build_with_surface` uses new API.
- `tur-android/src/lib.rs` — JNI trampolines call new handle methods.
- `tur-native/...` — same.
- `tur-integration-tests/src/lib.rs` — test harness uses new API.
- All `tur-integration-tests/tests/**` — replace `push_platform_event` / `run_frame` / `set_after_frame_hook` patterns.

## Verification

1. All existing 170 element + 93 event tests pass.
2. New unit tests for the message protocol (in `comm.rs` or `lib.rs`).
3. `cargo build --workspace` clean.
4. `cargo clippy --workspace -- -D warnings` clean.
5. `cargo check --target wasm32-unknown-unknown` clean.

## ✅ Outcome (refined scope)

The plan's original scope ("convert every today's direct `TurApp` pub fn into channel messages" with full embedder + test churn) was overkill for what Phase 5/6/7 actually need. The refined scope delivers the wire format and the dispatch pattern that unblock Phase 5 (event bus / async APIs / escape-hatch removal) and Phase 6 (Send + Sync prep) without rewriting embedder/test code.

**What landed:**
- `core/app/comm.rs` — `WorkerMsg`, `MainMsg`, `DevReply`, `ModuleError`, `Reply<T>` / `ReplySender<T>` (minimal `Arc<Mutex<Option<T>>>` slot pair; no tokio/futures dep — engine core stays tokio-free; Phase 7 swaps for `tokio::sync::oneshot` to enable `.await`).
- `TurApp::handle_worker_msg(msg: WorkerMsg)` — single internal dispatch. Every existing pub fn (`push_platform_event`, `push_app_event`, `request_paint`, `load_module`, `load_js`, `eval_module`) now builds a `WorkerMsg` and dispatches through this method (single source of truth; Phase 7's worker task body is `while let Some(msg) = rx.blocking_recv() { app.handle_worker_msg(msg) }`).
- `TurApp::pump() -> Result<FrameOutcome, TurError>` — canonical frame entry (alias: `run_frame`). Matches the Phase 4+ main-thread vocabulary ("pump" = drive one frame).
- `ModuleError` → `TurError` conversion (`error.rs`).
- `CustomPlatformEvent` / `CustomAppEvent` traits gain `Send + Sync` supertraits — small Phase 6 preview needed so `PlatformEvent` (carried by `WorkerMsg::PlatformEvent`) is `Send`. All three current implementors (`ClipboardPlatformPasteEvent`, `ClipboardPasteEvent`, `ClipboardWriteEvent`, `ScrollFlingEvent`) are plain data, so no impl changes needed.
- Compile-time `Send` assertions on the wire types in `comm.rs`.
- 4 new in-crate unit tests (Arc<str> source carrier, ModuleError Display, Reply round-trip, dropped sender).

**What was deferred to Phase 5/6/7:**
- Async `load_and_run_module(Arc<str>)` — Phase 5 converts sync→async once the event bus refactor lands.
- `MainMsg` emission — Phase 7 wires actual mpsc channels and per-frame drain.
- `WorkerMsg::AppEvent(AppEvent)` variant — Phase 6 adds the bound.
- Removing `with_app` / `with_boa_context` / `with_element` escape hatches — Phase 5.
- Per-embedder migration (tur-wasm/tur-android/tur-native rAF loop rewrite) — Phase 7 (when channels are real).

**Verification:**
- All 170 element + 93 event + 8 vello tests pass (no embedder/test changes needed).
- 27 in-crate unit tests pass (4 new from comm.rs).
- `cargo build` / `cargo clippy --workspace` clean under both `direct-render` feature configs.
- `cargo check --target wasm32-unknown-unknown -p tur-wasm` clean.

## Risks

- **API breakage is wide** — every embedder and every test file touches the changed APIs. This is the biggest "API break" phase. Land as one PR with full test migration.
- **Async load_and_run_module** — many tests today call `load_module` then immediately query state. They must now `.await` (or use the test harness's synchronous wrapper, which sends + blocking_recv on the oneshot).
- **rAF loop rewrite** — each embedder's frame loop changes shape. Verify wasm + Android still render correctly.

## Out of scope

- Multi-threading — channels are in-process.
- Event bus refactor — Phase 5.
- Tokio runtime injection — Phase 7.

## Estimated scope

- ~400 lines new (comm.rs, handle restructure)
- ~600 lines modified (every embedder + every test)
- Single large PR; budget 2-3 days for review feedback
