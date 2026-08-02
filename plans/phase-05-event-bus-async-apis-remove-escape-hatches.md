# Phase 5 — Event bus always-installed; async module/dev APIs; remove escape hatches

**Status:** ✅ partial (refined scope — see "Outcome" below)
**Prerequisite:** Phase 4 (channel-based TurApp API — wire format defined).
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

## ✅ Outcome (refined scope)

The original Phase 5 scope bundled three large changes (event bus promotion + async APIs + escape hatch removal) into one massive API-break phase. The refined scope delivers the event bus promotion now (the only piece that has visible single-threaded value) and defers the other two to Phase 7, where they align naturally with the actual multi-threaded migration.

**What landed:**
- `TurApp::event_bus() -> EventBus` — non-Option direct handle (the bus is unconditionally installed by `TurStdPlugin`, so the historical `Option` was always unwrappable in practice).
- `EventBus::of(&app)` kept as back-compat alias returning `Some(app.event_bus())` — zero test churn.
- `EventBus::from_inner(Rc<EventBusInner>)` constructor exposed so the crate root can build a handle from the engine's instance data.
- `TurApp::focused_state() -> FocusedState` — combined accessor (`is_editable + cursor_rect` in one call). Replaces the two-call `focused_is_editable()` + `focused_cursor_rect()` pattern when the caller needs both. Today this reads live from engine state; Phase 7's worker will populate a main-side cache via `MainMsg::FocusedStateChanged`.
- New `FocusedState` struct at the crate root.

**What was deferred to Phase 7:**
- **Async module/dev APIs** (`async fn load_and_run_module`, etc.) — these require an async runtime to `.await`. The engine core is tokio-free today; Phase 7 introduces `TurRuntimeBuilder::tokio_handle(handle)`, at which point async APIs become natural. The wire types are already defined (Phase 4's `WorkerMsg::LoadModule { reply: ReplySender<...> }`); the sync wrapper in `TurApp::load_module` continues to work.
- **Escape hatch removal** (`with_app` / `with_boa_context` / `with_element`) — these are used by ~40 test files and 2 embedder crates for introspection. They work fine single-threaded (closures don't cross threads). Removing them is a Phase 7 concern: when the worker/main split goes live, closures can't cross the boundary, so they must be replaced with named handles. Phase 6's Send+Sync prep doesn't require their removal — Phase 6 adds bounds to wire types and capability traits, not to `TurApp` itself (which stays main-side).

**Why defer rather than do it now?**
- **Avoid test/embedder churn twice.** Doing the async + escape-hatch migration now, then re-doing it in Phase 7 for the channel wiring, doubles the work. Phase 7 will be a single coordinated migration: channels + async + escape hatch removal, all in service of the worker/main split.
- **Phase 6 unblocked.** Phase 6's Send+Sync bounds only need Phase 4's wire types (already done) and Phase 5's event bus promotion (now done). It doesn't need escape hatches gone.

**Verification:**
- All 170 element + 93 event + 8 vello tests pass (no test changes needed).
- 27 in-crate unit tests pass.
- `cargo build` / `cargo clippy --workspace` clean under both `direct-render` feature configs.
- `cargo check --target wasm32-unknown-unknown -p tur-wasm` clean.

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
