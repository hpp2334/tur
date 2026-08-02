# Phase 8 — Production threaded runtime + embedder migration

**Status:** 🚧 planning — Phase 7 (trait abstraction + ThreadedBackend) landed; this phase wires it into production
**Prerequisite:** Phase 7 (`TurAppBackend` trait, `ThreadedBackend`, cross-thread RPC for most methods)
**Goal:** Native embedders (`tur-android`, `demo/compose`) opt into threaded mode via `TurRuntime::create_app_threaded(...)`. JS engine + layout run on a worker thread; renderer + scheduling stay on main. Wasm stays single-threaded until Phase 9.

## Why this is a separate phase

Phase 7 delivered the **threading infrastructure** (worker thread + mpsc channels + RPC variants). What remains is **runtime integration**: the runtime today holds `Rc<dyn Clock>` / `Rc<dyn FontLoader>` / `Capabilities` (all `Rc`-backed, `!Send`). To spawn a worker that constructs engine state from runtime config, the config must cross the thread boundary — which means either Arc migration or a factory pattern.

## ✅ Already landed (Phase 7, end-of-session state)

`ThreadedBackend` implements `TurAppBackend` with cross-thread RPC for:
- `pump` (Wake + `MainMsg::FrameOutcome`)
- `load_module` / `load_js` / `eval_module` (Reply slot)
- `dev_tool_element_tree` / `dev_tool_get_element` (Reply slot)
- `focused_state` / `focused_element` / `focused_cursor_rect` / `focused_is_editable` (Reply slot — wasm hot path)
- `query_element` (Reply slot)
- `push_app_event` (fire-and-forget via `WorkerMsg::AppEvent`)
- `render_to_pixels` (Reply slot)
- `request_paint` / `push_platform_event` (via `handle_worker_msg`)
- `handle_worker_msg` (dispatch)

Public API: `TurApp::new(Box::new(ThreadedBackend::new(factory_closure)))` where `factory_closure: impl FnOnce() -> InlineBackend + Send + 'static`. The factory runs on the worker thread (so it can construct `!Send` `Rc`/`boa::Context` locally).

`build_inline_backend(...)` extracted as a pub helper — both inline `create_instance` and the threaded factory call it.

Smoke test (`libs/tur-integration-tests/tests/element/threaded_backend.rs`) proves cross-thread dispatch for all RPC variants.

## 🚧 Remaining — three independent workstreams

### 8.1 Runtime integration (the big rock)

Add `TurRuntime::create_app_threaded` so embedders don't have to construct the factory themselves. Two architectural options:

**A. Factory closures on the builder (least invasive)**

Add to `TurRuntimeBuilder`:
```rust
.clock_factory(Box<dyn Fn() -> Rc<dyn Clock> + Send + Sync>)
.font_loader_factory(Box<dyn Fn() -> Rc<dyn FontLoader> + Send + Sync>)
```

Embedder provides both `.clock(...)` (for inline) AND `.clock_factory(...)` (for threaded). The runtime uses whichever is set based on `create_app` vs `create_app_threaded`. Awkward but localized.

**B. Migrate runtime to Arc + Send + Sync (the right long-term path)**

Change `clock: Rc<dyn Clock>` → `clock: Arc<dyn Clock + Send + Sync>`. Same for `font_loader`, `capabilities` internals. Embedders update from `Rc` to `Arc` at construction. The runtime is then `Send` and can produce a factory closure itself.

```rust
// The holy grail API:
impl TurRuntime {
    pub fn create_app_threaded(
        self: &Rc<Self>,
        renderer_factory: impl FnOnce() -> Box<dyn Renderer> + Send + 'static,
        viewport: (f64, f64),
    ) -> Result<Rc<TurApp>, TurError> {
        let clock = self.clock.clone();         // Arc<dyn Clock + Send + Sync>
        let font_loader = self.font_loader.clone();
        let capabilities = self.capabilities.clone();
        let plugins = self.plugins.clone();      // Vec<Box<dyn Plugin + Send + Sync>>
        let backend_factory = move || {
            build_inline_backend(clock, font_context, font_loader, capabilities,
                                 &plugins, renderer_factory(), viewport)
        };
        let backend = ThreadedBackend::new(backend_factory);
        Ok(Rc::new(TurApp::new(Box::new(backend))))
    }
}
```

**Migration cost (Option B):**
- `Rc<dyn Clock>` → `Arc<dyn Clock + Send + Sync>` in `TurRuntime` + `TurRuntimeBuilder` (~5 sites)
- `Rc<dyn FontLoader>` → `Arc<dyn FontLoader + Send + Sync>` (~5 sites)
- `Capabilities` internals: `Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>` → `Arc<Mutex<...>>`. The `Box<dyn Any>` values need to be `Send` — verify each capability impl satisfies this (Clipboard/Http/FilePicker backends).
- Plugin Vec cloning: `Vec<Box<dyn Plugin>>` doesn't impl Clone. Need either `Arc<Vec<Arc<dyn Plugin>>>` or a clone helper.
- Embedder updates: each `TurRuntimeBuilder::clock(Rc::new(...))` becomes `.clock(Arc::new(...))`. Touches tur-wasm (~3 sites), tur-android (~3 sites), tur-native, demo/compose (~3 sites), tests.
- The boa `Clock` trait doesn't have `Send + Sync` supertrait today; embedder clocks (`StdClock`, `FixedClock`, `WasmClock`) need bounds added. StdClock/FixedClock are Send + Sync already; WasmClock needs verification.

Estimated scope: ~300 LOC + ~30 call sites. One focused day.

**Recommendation: Option B.** Cleaner, no API duplication, sets up Phase 9 (wasm threads) cleanly.

### 8.2 Cross-thread EventBus (optional — defer unless needed)

The remaining `ThreadedBackend` panic. Today's EventBus uses `Rc<RefCell<...>>` for queues — `!Send`.

**Architecture:**
```rust
pub struct EventBus {
    host_to_js: Arc<Mutex<VecDeque<Vec<u8>>>>,    // ← was RefCell
    js_to_host: Arc<Mutex<VecDeque<Vec<u8>>>>,    // ← was RefCell
    js_handlers: RefCell<Vec<JsFunction>>,         // worker-only (JsFunction !Send)
    host_handlers: RefCell<Vec<HostHandler>>,      // worker-side (subsystem invokes)
}

#[derive(Clone)]
pub struct EventBusHandle {
    host_to_js: Arc<Mutex<VecDeque<Vec<u8>>>>,
    js_to_host: Arc<Mutex<VecDeque<Vec<u8>>>>,
}
```

`TurAppBackend` gains `event_bus_handle() -> EventBusHandle`. For inline, constructed from `internal.event_bus.handle()`. For threaded, constructed at worker-spawn time with Arc queue clones shared with the worker's EventBus.

`TurApp::event_bus()` keeps today's signature (`Rc<EventBus>`) for inline-only full-API access (tests). Production threaded uses `event_bus_handle()` (limited to `emit_to_js` + `drain_js_to_host`).

**Defer unless:** production threaded embedder actually needs the bus. The wasm website passes `after_frame: None`, so it doesn't. Native threaded embedders (8.3) might want it for IPC.

### 8.3 Cross-thread set_cursor_backend (needed for wasm threaded — Phase 9)

For native threaded (8.1 lands first), no production use yet. For wasm threaded (Phase 9), the website does call `set_cursor_backend` after `create_app`.

**Architecture:** worker emits `MainMsg::CursorChanged(Cursor)` when the engine resolves a new cursor during flush. Main's pump drains and applies to its `CursorBackend` (stored on main, not shipped to worker).

Requires:
- A "RecordingCursor" wrapper on the worker that captures cursor changes during flush
- InlineBackend exposes the latest recorded cursor after flush
- ThreadedBackend.pump reads it, ships via MainMsg::CursorChanged, drains on main, applies to backend
- `set_cursor_backend` stores on main (no longer ships to worker)

**Defer to Phase 9** unless native threaded production use emerges first.

## 8.4 Embedder migration order

1. **`tur-native`** — simplest, ~10 LOC. Update `clock`/`font_loader` construction to Arc. Add a threaded mode flag.
2. **`demo/compose` (Android)** — ~30 LOC. Same Arc migration. Test on device.
3. **`tur-android`** — ~50 LOC. JNI thread coordination. The JNI event bridge already runs on a separate thread; the worker is just one more.
4. **`tur-wasm`** — Phase 9 (needs wasm-bindgen-rayon + COOP/COEP).

## 8.5 Risks

- **boa Context construction time on worker** — first pump may stall. Mitigation: construct eagerly at `create_app_threaded` (block on join) OR show a loading state.
- **Per-frame RPC latency** — focused_state etc. round-trip per frame. Mitigation: cache on main from `MainMsg::FocusedStateChanged` (emit on change, not per frame).
- **Capability Send verification** — each backend (Clipboard/Http/FilePicker) must be `Send + Sync` for the runtime to be Arc-migrated. Verify in a dedicated test.

## Estimated scope

- 8.1 (runtime Arc migration + `create_app_threaded`): ~300 LOC, ~30 call sites, 1 day
- 8.2 (cross-thread EventBus): ~200 LOC, defer unless needed
- 8.3 (cross-thread cursor): ~150 LOC, defer to Phase 9
- 8.4 (embedder migration): ~100 LOC per embedder, 1-2 days

**Total Phase 8 (without 8.2/8.3 deferrals): ~400 LOC + 1-2 days**

## Out of scope

- Phase 9 (wasm threads) — depends on Phase 8 landing first.
- Public API changes for embedders — `create_app_threaded` is additive; existing `create_app` continues to work for inline mode (default).
