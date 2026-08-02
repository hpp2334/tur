# Phase 6 — `Send + Sync` prep (CANCELLED — replaced by per-worker model)

**Status:** ❌ cancelled — superseded by Phase 7's "local thread shared" model
**Prerequisite:** Phase 5.
**Goal:** ~~convert every `Rc`-shared cross-thread type to `Arc`, add `Send + Sync` supertraits to `Plugin` and capability backend traits, simplify `NativeHttp::new()`.~~ **No longer required.**

## Background

The original Phase 6 plan assumed capability backends (Clipboard/Http/FilePicker) would be **shared across worker threads** in Phase 7 — meaning the same `Arc<dyn Backend + Send + Sync>` is invoked from multiple OS threads concurrently. That requires `Send + Sync` everywhere.

After reviewing the actual backend shapes (most are stateless dispatchers — `WasmClipboard` calls `navigator.clipboard`, `WasmHttp` builds a fresh reqwest client per request, etc.), we adopted a **"local thread shared"** ownership model instead:

- Each worker thread constructs its **own** backend set (cheap — unit structs or thin handles).
- Backends are shared **within** a worker (across instances pinned to that worker) via `Rc<dyn Backend>` — same as today.
- Across worker threads: no sharing. Each worker has its own `Rc`-rooted backend graph.
- The runtime stores **factories** (or callable constructors), not backends. At worker-thread startup, the worker calls the factories to build its own backends.

## Consequences

- **No `Arc`/`Send + Sync` conversion needed.** Backends stay `Rc<dyn Backend>`. Plugin trait stays without `Send + Sync` bounds. Capabilities map stays `Rc<RefCell<>>`.
- **Phase 6 is a no-op.** Skipped entirely.
- **Phase 7 grows slightly.** It must implement the per-worker factory pattern: the runtime hands each worker a `RuntimeConfig` containing backend factories, and the worker constructs its `Rc`-rooted graph on first spawn.
- **`NativeHttp::new(handle)`** keeps its `Handle` parameter — the factory just clones the `Handle` per worker.

## What was kept (already landed in earlier phases)

- `CustomPlatformEvent` / `CustomAppEvent` `Send + Sync` bounds (added in Phase 4 so `WorkerMsg::PlatformEvent` could be `Send`). Still needed: `PlatformEvent` crosses the main→worker channel regardless of backend ownership model.
- Compile-time `Send` assertions on `WorkerMsg` / `MainMsg` / `DevReply` / `ModuleError` (Phase 4).
- `WorkerMsg` / `MainMsg` are `Send` (Phase 4).

## Risks

- **Per-worker backend construction cost.** Negligible for unit-struct backends; `arboard::Clipboard` construction is cheap; tokio `Handle::clone()` is an `Arc` bump. One-time cost per worker thread, amortized across all instances on that worker.
- **No cross-worker backend state sharing.** If a backend ever needs to share state across instances on different workers (e.g. a shared in-memory cache), it must use its own internal `Arc<Mutex<>>` — not the engine's responsibility. None of today's backends need this.

## Out of scope

- Per-worker factory pattern — Phase 7.
- Removing the master `Capabilities` map at the runtime level — Phase 7 (each worker builds its own).
