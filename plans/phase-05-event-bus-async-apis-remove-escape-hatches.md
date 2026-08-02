# Phase 5 — Event bus always-installed; async module/dev APIs; remove escape hatches

**Status:** not started
**Prerequisite:** Phase 4 (channel-based TurApp API).
**Goal:** three coordinated changes that all touch the public API surface:
1. Promote the event bus from opt-in plugin to always-installed builtin.
2. Convert module-loading and dev-tool APIs to real Rust async fns.
3. Remove `with_app`, `with_boa_context`, `with_element` — no closure-based escape hatches across the worker boundary.

## Background

After Phase 4, `TurApp` is a handle with mpsc channels. Three loose ends remain:

- The event bus is registered by `TurStdPlugin` (currently always installed there, but the API surface still treats it as optional: `EventBus::of(&app) -> Option<EventBus>`). Drop the `Option`.
- `load_and_run_module` etc. are conceptually async but Phase 4 may have left them as fire-and-forget + callback. Make them real `async fn`.
- `with_app` / `with_boa_context` / `with_element` accept arbitrary closures — those can't cross a thread boundary. Remove them; replace with named handles (only the event bus needs this today).

## Changes

### 5.1 — Event bus always-installed

Today: `install_event_bus` is called from `TurStdPlugin::register` at `builtin_plugins/std.rs:131`. The handle retrieval `EventBus::of(&app) -> Option<EventBus>` (at `event_bus/mod.rs:66-68`) wraps `app.instance_data::<EventBusInner>()`.

Tomorrow:
- Keep `install_event_bus` (no longer "optional" — already always called).
- Add `TurApp::event_bus(&self) -> EventBus` (non-Option). Returns the main-side handle directly.
- The main-side `EventBus` handle wraps the comm channels:
  ```rust
  pub struct EventBus {
      worker_tx: mpsc::Sender<WorkerMsg>,
      main_rx_buffer: Rc<RefCell<VecDeque<Vec<u8>>>>,  // populated by main's per-frame pump
  }
  impl EventBus {
      pub fn emit_to_js(&self, bytes: Vec<u8>) {
          let _ = self.worker_tx.send(WorkerMsg::EventBusToJs(bytes));
      }
      pub fn try_recv(&self) -> Option<Vec<u8>> { self.main_rx_buffer.borrow_mut().pop_front() }
      pub fn on_message<F: Fn(&[u8]) + 'static>(&self, handler: F) { ... }
  }
  ```
- JS→host messages arrive as `MainMsg::EventBusToHost(bytes)` — main's per-frame pump dispatches them to the EventBus handle's buffer/handlers.

### 5.2 — Async module-loading APIs

```rust
impl TurApp {
    pub async fn load_and_run_module(&self, source: Arc<str>) -> Result<(), ModuleError> {
        let (tx, rx) = oneshot::channel();
        self.worker_tx.send(WorkerMsg::LoadModule { source, reply: tx }).ok()?;
        rx.await.map_err(|_| ModuleError::WorkerGone)?
    }
    pub async fn load_js(&self, source: Arc<str>) -> Result<(), ModuleError> { /* same */ }
    pub async fn eval_module(&self, source: Arc<str>) -> Result<(), ModuleError> { /* same */ }
}
```

The returned future is `Send` (oneshot receiver is Send). Embedder can `.await` directly on main, `tokio::spawn` it, or wrap in a JS Promise via `wasm-bindgen-futures` / boa's `JsPromise::from_async`.

### 5.3 — Async dev-tool queries

```rust
impl TurApp {
    pub async fn dev_tool_element_tree(&self) -> DevNodeData {
        let (tx, rx) = oneshot::channel();
        let _ = self.worker_tx.send(WorkerMsg::DevElementTree { reply: tx });
        rx.await.unwrap_or_default()
    }
    pub async fn dev_tool_get_element(&self, id: NodeId) -> Option<DevNodeData> { /* same */ }
}
```

JS bridge: `turDevTool.elementTree()` returns a `Promise`. Bridge awaits the Rust future via `wasm-bindgen-futures::future_to_promise` on wasm.

### 5.4 — Remove escape hatches

- `TurApp::with_app` (tur-android `lib.rs:534`) — removed.
- `TurApp::with_boa_context` (`lib.rs:148`) — removed.
- `TurApp::with_element` (`lib.rs:296`) — removed.
- `TurApp::instance_data::<T>()` — kept but only callable from worker-internal code (not pub). Embedders use named handles (`event_bus()`).

If a future plugin needs main-side custom data flow, follow the EventBus pattern: define a named handle type, expose it via a `TurApp::your_handle()` method.

### 5.5 — Hot-path state push (already in Phase 4 channels, formalized here)

Worker emits `MainMsg::FocusedStateChanged { is_editable, cursor_rect }` only when the value differs from the previous frame. Main caches; `TurApp::focused_is_editable()` / `TurApp::focused_cursor_rect()` read from cache — no RPC.

```rust
struct FocusedState {
    is_editable: bool,
    cursor_rect: Option<(f64, f64, f64, f64)>,
}
```

Worker tracks `last_focused_state` and emits only on change.

## Files to modify

- `libs/tur-engine/src/lib.rs` — add `event_bus()`, async module/dev APIs; remove escape hatches; add `FocusedState` cache.
- `libs/tur-engine/src/builtin_plugins/event_bus/mod.rs` — main-side `EventBus` handle type; remove `of()`'s `Option` (or replace with a non-Option `of()`).
- `libs/tur-engine/src/builtin_plugins/std.rs` — comment update (event bus install is no longer "optional" — already unconditional, just clarify).
- `libs/tur-android/src/lib.rs` — remove `ops::with_app`. Provide replacement (embedder uses `app.event_bus()` directly).
- `demo/website/native/src/lib.rs` — update if it used any removed APIs.
- `libs/tur-integration-tests/**` — update tests that called `with_boa_context` / `with_element` / sync module loading.

## Verification

1. `cargo test --workspace` — all tests pass with new async patterns.
2. Manual smoke test: playground's run button still works (async module load).
3. Event bus integration test (`tests/element/event_bus.rs`) passes with the new handle.
4. `cargo clippy --workspace -- -D warnings`.
5. `cargo check --target wasm32-unknown-unknown`.

## Risks

- **Async test refactoring** — every test that loads a module + queries state must await. Test harness needs an async wrapper or `block_on`.
- **Embedder breakage** — `with_app` was an escape hatch for JNI; removing it forces embedders to use named handles. The demo cdylib must update.
- **Event bus handler timing** — today handlers run inline during flush; tomorrow they run on main during the per-frame pump. Slight timing shift (1 frame max). Document this.

## Out of scope

- Multi-threading — still single-threaded (in-process channels).
- Tokio runtime injection — Phase 7.

## Estimated scope

- ~200 lines new (EventBus main handle, async API bodies)
- ~400 lines modified (escape hatch removal across engine + embedders + tests)
- API-breaking; coordinate with downstream embedders in one PR
