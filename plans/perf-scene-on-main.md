# Perf fix — Scene / Resources / MainTree on main; eliminate overhead + `block_on` parking

Branch: `refactor/scheduler` (continues the worker/main split work).

## 1. Profile evidence (two profiles)

### Chrome DevTools trace (macOS) — `Trace-20260804T094100.json.gz`

3.66 s / 190 frames → **51.8 fps**, 18 dropped (8.7 %).

- **Worker thread: completely idle.** Only V8 interrupt ticks (`HandleInterrupts` / `StackGuard` at 0.00 ms). The record pass is **negligible** — `RecordingCanvas` captures light `CanvasOp` enum variants with `Arc`'d heavy data (`FillTextLayout { layout: Arc<TextLayoutData> }`, `command.rs:70`). Confirms the user's point: the record pass is not a "second encode."
- **Main thread: ~17 ms opaque `RunMicrotasks`** per frame (wasm). Dead time between frames: **~0.7 ms** (input + rAF + compositor). The main thread is ~96 % saturated by the wasm block.
- **GPU process: ~17 ms `RunTask`**, **overlapping** main by 10-16 ms (WebGL2 command pipelining works). Not an additional bottleneck.
- `DrawingBuffer::prepareMailbox`: 0.001 ms — negligible.

**What the 17 ms IS:** the vello Scene encode on main (`scene.reset()` + `play_commands` → `VelloPaintContext` → `glyph_run` atlas-OFF → `fill_path` → `StripGenerator::generate_filled_path` per glyph) + WebGL2 command submission. This is the **same heavy work** single-threaded does — the multi-threading adds only ~0.7 ms overhead (record + wire + clones).

### Firefox Profiler (Windows / Ryzen 7 9700X) — `Firefox 2026-08-04 21.40 profile.json.gz`

~2.96 s of samples (1 ms interval). **stackwalk=0** (no native stack walking — many frames are unsymbolicated hex).

- **25.4 % in `ZwWaitForAlertByThreadId`** (thread parking / `Atomics.wait`). Temporal analysis: **324 ms at startup** (idle before animation), **~1094 ms at end** (idle after animation). During animation: **~2 ms every ~7 frames** — periodic, not per-frame blocking. The parking dominates the profile aggregate but is mostly start/end idle.
- **68.4 % unsymbolicated hex** — the actual wasm work, invisible without stackwalk.
- **5 % symbolicated wasm** — tiny per-function inclusive times (6-8 ms each: `StripGenerator::generate_filled_path`, `Scene::fill_path`, `OutlineGlyphCollection::get`, `WebGlRenderer::render`). Consistent with the encode being spread across many inlined functions.
- The `block_on` executor on the worker loop causes the parking noise — it makes the worker thread's profile show huge `.parking()` / `ZwWaitForAlertByThreadId` blocks that obscure the real work (see Phase 6).

**Cross-profile conclusion:** the worker is genuinely fast (idle in Chrome, brief periodic parking in Firefox). The frame cost is the **main-thread vello encode + WebGL render** — same as single-threaded. The multi-threading overhead is ~0.7 ms/frame, not the dominant cost.

**Main-branch paint analysis (verified against `../tur`):** main has **zero paint-level caching or dirty-node tracking**. Within a rendered frame it's naive: `scene.reset()` (`scene_paint.rs:50`) + full-tree-walk (`element_tree.rs:571-634`) every time. No scene retention, no per-node paint dirty flag (`dirty_layout` is layout-only — `element_object.rs:19`), no CanvasOp cache, no path cache, no invisible-node culling (Opacity=0 still paints children — `effects/render.rs:14-27`). The only paint-cost gates are: (a) frame-level skip-when-idle (`internal.rs:311`), and (b) layout subtree skip (`element_tree.rs:510`). vello_hybrid's `Resources` persists the GPU glyph/image atlas, but since glyph_run runs **atlas-OFF**, the glyph atlas path isn't exercised — glyphs re-flatten as outline paths (`StripGenerator::generate_filled_path`) every frame. **The 17 ms encode cost is inherent to the immediate-mode full-repaint model — identical in single and multi-threaded.**

The plan addresses the multi-threading overhead (~0.7 ms) + profiling clarity (`block_on` parking). The encode cost itself is a separate problem (would need atlas ON or per-node dirty paint tracking — both deferred).

## 2. Root causes

**Key finding (verified against `main`):** the single-threaded version does the *exact same* per-frame encode work — full-tree paint, Scene `reset()` + full re-encode, glyph_run atlas-OFF — in one pass. The Scene is **not** long-lived or diffed: vello_hybrid is immediate-mode (`scene.reset()` + full re-encode every frame in both `main`'s `paint_tree_to_scene` and this branch's `paint_commands_to_scene`). Glyph atlas is OFF in both modes — **kept OFF** per user decision.

The multi-threading adds a small but measurable tax on top of the single-threaded baseline:

1. **Serialized worker→main (0.7 ms/frame overhead).** The record pass is **light** (Arc'd heavy data), but with no pipelining (1-in-flight), the worker's flush+record runs before main can replay. The ~0.7 ms dead time (Chrome) is the serialization gap. Pipelining recovers this — but it's ~1 ms, not the ~8 ms earlier claimed (the "double encode" framing was wrong: record ≠ encode).
2. **Per-frame O(nodes) allocations (light but unnecessary).** `MainTree.apply_batch` (`internal.rs:357`) clones every node's `ops` (`Vec<CanvasOp>` → `clone_from` at `main_tree.rs:98`) — light per-op (Arc'd heavy data) but still N Vec allocations — on the **worker**, for a mirror the render path never consults. `build_topology_batch` emits `SetChildren` for every node every frame (N `Vec<ElementNodeId>` allocations). `Arc<ImageResourceMap>` is refcount-bumped + shipped every frame.
3. **`block_on` on the worker loop (profiling clarity).** `worker_sched.block_on(worker_loop)` (`backend.rs:496`) drives the async loop via a futures executor. When idle, the executor parks the thread — showing up as huge `ZwWaitForAlertByThreadId` / `.parking()` blocks in profiles (25.4 % of the Firefox aggregate), obscuring the actual worker work. This makes profiling and optimization difficult.
4. **Per-frame `resize()` in the render_sink + unnecessary callback indirection.** The render_sink callback (`tur-wasm/src/app.rs:430`, `tur-android/src/app.rs:240`, `tur-integration-tests/tests/vello/vello_app.rs:134`) calls `r.resize(lw, lh, vdpr)` unconditionally before every `r.render_commands(...)` — **every frame**, even when the viewport hasn't changed. `resize()` is not cheap: it recreates the `Scene` (`webgl_renderer.rs:145` / `renderer.rs:118`), and on wgpu reconfigures the GPU surface (`renderer.rs:114`). Then `paint_commands_to_scene` calls `scene.reset()` (`scene_paint.rs:45`) on the just-allocated fresh scene — **double-clear**. In `main`, the engine calls `renderer.resize()` **only** when handling `PlatformEvent::Resize`. **Deeper issue:** the render_sink callback itself is unnecessary indirection. The renderer lives on main; `MainBackend` (which receives `MainMsg::RenderCommands`) also lives on main. In `main`, `create_app(Box::new(renderer), ...)` — the engine owns the renderer and calls it directly. This branch introduced the callback as a refactor artifact of the worker/main split, but since `MainBackend::pump()` is on main, it could own `Box<dyn Renderer>` and call it directly. **Fix:** `create_app(Box<dyn Renderer>, viewport, dpr)` — MainBackend owns the renderer. No callback, no `set_render_sink`, no per-frame `resize()`. See Phase 6.

## 3. Target architecture

```
WORKER (CPU)                              MAIN (CPU + GPU)
─────────────────                         ─────────────────────────────────
boa Context                               MainTree (retained, per-node ops)
element tree                              Resources (warm: image atlas)
reactive store                            Scene (immediate-mode: reset + encode/frame)
layout                                    VelloRenderer (GPU)
subsystems                                render_sink
RecordingCanvas (capture)
                                          wire = pure data (Vec<RenderCommand> / diff)
flush → record ──────────────────────►    apply batch → MainTree
              (pipelined: latest-wins)      │
                                            ▼
                                          if needs_paint: reset Scene,
                                            encode from batch (glyph_run atlas-OFF),
                                            GPU render
                                          owns ImageResourceMap + uploads
```

**Everything GPU-coupled (Scene, Resources, image atlas, MainTree) lives on main, warm, and never crosses threads.** Glyph atlas stays OFF.

## 4. Key design decisions

### 4.1 Scene + Resources on main
The `Scene` is the GPU render artifact; it belongs where the GPU is. vello_hybrid's `Scene` is immediate-mode (`reset()` + full encode per frame), so there is no retained-mode win by moving it. **Decision: Scene + Resources stay on main.**

### 4.2 MainTree on main
Currently on the worker (`internal.rs:86`) as a Phase-3 artifact. Move to **main** as the retained authority for dev-tool/hit-test/cursor. **Decision: move `MainTree` to `MainBackend`.**

### 4.3 RecordingCanvas stays on the worker
Stays as the capture mechanism. Phase 5 addresses the `block_on` parking **only paint-dirty nodes** (driven by the existing `dirty_subscribers` signal from `flush_reactive`, `internal.rs:396`).

### 4.4 Glyph atlas stays OFF
`main` runs atlas-OFF and the profiles confirm the cost is in the encode, not a multi-threading artifact. **Decision: atlas stays OFF.** Future encode-cost optimization (atlas ON, fewer nodes, etc.) is independent of this plan.

### 4.5 Image map on main
Main owns `ImageResourceMap` + image atlas + `image_uploads`. Worker keeps a thin `{ImageResourceId → (Size, Option<ImageId>)}` cache. `createImageResource`: worker decodes, assigns id (sync JS return preserved), ships `(id, ImageResource)` once. No `Arc<ImageResourceMap>` crosses the wire after the first upload.

### 4.6 `block_on` → `spawn_local` on wasm (event-loop-driven, no parking)
`block_on` calls `futures::executor::block_on` (`scheduler.rs:164`), which parks the Web Worker via `Atomics.wait` → `ZwWaitForAlertByThreadId` when the future yields. Replace with `spawn_local` — the future runs on the JS event loop, yielding between messages with no parking. A sync `recv()` pump would have the **same** parking problem; the fix is to keep the async loop, just drive it on the event loop. See Phase 6.

### 4.7 Pipelining recovers ~1 ms, not ~8 ms
The earlier "~17 ms → ~9 ms" claim was based on the wrong "double encode" framing. The record pass is light (Arc clones), and the profiles confirm the worker is idle/negligible. Pipelining recovers the ~0.7 ms serialization gap + enables future worker-side optimizations (diff recording). It's still worth doing but is no longer the headline fix.

### 4.8 MainBackend owns the renderer (no render_sink callback)
In `main`, `create_app(Box::new(renderer), ...)` — the engine owns the renderer and calls it directly. This branch introduced `set_render_sink` as a refactor artifact: the engine moved to a worker, the renderer stayed on main, and a callback bridged them. But `MainBackend` (which receives `MainMsg::RenderCommands` via `pump()`) is **also on main** — there is no thread boundary to bridge. **Decision:** `create_app(Box<dyn Renderer>, viewport, dpr)`; MainBackend owns the renderer, calls `render_commands()` / `resize()` / `present()` / `render_to_pixels()` directly. Delete `set_render_sink` / `render_sink` entirely. This also fixes the per-frame `resize()` bug (MainBackend calls `resize()` only on `PlatformEvent::Resize`, matching `main`). See Phase 6.

## 5. Phased implementation

Each phase is independently shippable + measurable.

---

### Phase 1 — Pipelining (2-in-flight)

**Goal:** overlap the worker's flush+record with main's encode+render. Recover the ~0.7 ms serialization gap; enable future worker-side optimizations.

**Model:** at vsync N, main takes the **latest** shipped batch (latest-wins — drop stale), encodes+renders it, and sends `Wake` for frame N+1. The worker records N+1 while main renders N.

**Changes:**
- `backend.rs` `MainBackend::pump`: decouple from request/response. The worker already ships `MainMsg::RenderCommands` as soon as it records (`worker_loop`, `backend.rs:795`). Main drains `main_rx`, keeps only the latest, renders at vsync.
- `tur-wasm/src/lib.rs:247` (rAF loop) + `tur-android/src/app.rs` (JNI frame loop): restructure to take-latest + render + send-Wake.
- Backpressure: worker only records on `Wake`; main sends one per vsync → at most 1 ahead.

**Effect:** ~0.7 ms/frame recovered; dead time → ~0.

---

### Phase 2 — Move MainTree to main

**Goal:** `MainTree` lives on main. Eliminates the per-frame worker-side `apply_batch` clone.

**Changes:**
- `internal.rs`: remove `main_tree: RefCell<MainTree>` (line 86) and the worker-side `apply_batch` (line 357).
- `backend.rs` / `MainBackend`: add `main_tree: RefCell<MainTree>`. On `MainMsg::RenderCommands`, call `main_tree.apply_batch(&commands)` on main before `render_sink`.
- Dev-tool queries already serve from the worker's `NodeTreeData` (`backend.rs:229`), not `MainTree` — unaffected.

---

### Phase 3 — Topology diff

**Goal:** stop shipping `SetChildren` for every node every frame.

**Changes:**
- `main_tree.rs` `build_topology_batch`: emit `SetChildren` only when a node's child list actually changed (compare against `last_topology`). Emit `Remove` for unmounted ids.
- With `MainTree` on main (Phase 2), topology application happens on main.

**Effect:** steady-state → zero topology commands on the wire.

---

### Phase 4 — Image-map directive (main owns images)

**Goal:** main owns `ImageResourceMap` + image atlas; stop shipping `Arc<ImageResourceMap>` every frame.

**Changes:**
- `context.rs`: remove `image_resource_map` from `TurAppContext`. Worker keeps `ImageHandleCache: HashMap<ImageResourceId, (Size, Option<ImageId>)>`.
- `builtin_plugins/image/bridge.rs`: decode on worker, assign id, ship `(id, ImageResource)` once via `MainMsg::UploadImage`.
- Main: on `UploadImage`, insert + `renderer.upload_image` + ship `{id→ImageId}` delta back.
- `comm.rs`: `RenderCommands` loses `image_map` field; add `UploadImage` + `ImageIdsUpdated`.
- Layout: `compute_layout` takes a size-lookup instead of `&ImageResourceMap`.

---

### Phase 5 — Drive worker_loop via `spawn_local` (not `block_on`)

**Goal:** eliminate the futures-executor parking that dominates profiles (`ZwWaitForAlertByThreadId` / `.parking()`), so profiling shows actual worker work. Keep the async loop — just drive it on the event loop instead of via the blocking executor.

**Problem:** `worker_sched.block_on(worker_loop)` (`backend.rs:496`) calls `futures::executor::block_on(fut)` (`tur-wasm/src/scheduler.rs:164`). On a Web Worker, `block_on` parks the thread when the future yields (e.g. `worker_rx.next().await` returns `Pending`) — on wasm this means `Atomics.wait` → `ZwWaitForAlertByThreadId`. The worker is parked for the entire gap between messages, which:
- Shows up as 25.4 % of the Firefox profile aggregate.
- Obscures the actual worker work.
- A sync `recv()` pump would have the **same** problem (it also parks while waiting).

**Fix:** drive `worker_loop` via `spawn_local` instead of `block_on`:
```rust
// tur-wasm/src/scheduler.rs — WasmWorkerScheduler / WasmSchedulerDriver:
fn block_on(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
    // WASM: drive on the JS event loop, NOT the blocking executor.
    // The future yields to the event loop when idle (no parking).
    // The Web Worker stays alive: spawn_local roots the future in the
    // microtask queue; pending microtasks keep the worker's event loop spinning.
    wasm_bindgen_futures::spawn_local(fut);
}
```

The future runs on the event loop — between messages, the worker is simply **not running** (no code executing, no `Atomics.wait`, nothing for the profiler to sample). When a `postMessage` arrives (the `Wake` from main), the channel's waker fires a microtask that re-polls `worker_loop`. This is exactly how `spawn_local` already works on the **main** thread (the main thread never calls `block_on` — it's all event-loop-driven).

**Native:** the OS-level park (futex/condvar) is efficient and doesn't show as a profiling problem. `block_on` can stay on native, OR optionally migrate to a `spawn_local`-on-main-thread + `recv()`-on-worker hybrid. Not urgent.

**Effect:** clean profiles (no `ZwWaitForAlertByThreadId` parking noise), lower overhead (no executor poll/wake cycle). No frame-time change, but makes all future profiling actionable — the 68.4 % opaque-hex + 25.4 % parking becomes visible worker work.

**Risk:** `spawn_worker`'s closure (`scheduler.rs:110`) currently returns after `block_on` completes (the worker thread exits). With `spawn_local`, the closure returns immediately — the Web Worker stays alive only because `spawn_local` roots the future. Verify the worker doesn't terminate prematurely. `spawn_local` futures (JS Promise resolution, boa JobExecutor, async completions) already work this way on the main thread — no change needed.

---

---

### Phase 6 — MainBackend owns the renderer (delete render_sink; resize on event only)

**Goal:** eliminate the `render_sink` callback indirection. `create_app(Box<dyn Renderer>, ...)` — MainBackend owns the renderer and calls it directly. Resize fires only on `PlatformEvent::Resize`, matching `main`.

**Problem:** two issues, same root cause (renderer treated as external to the app):
1. **Unnecessary callback.** The renderer lives on main; `MainBackend::pump()` also lives on main. The `render_sink` callback (`set_render_sink`) bridges a gap that doesn't exist — both are same-thread. In `main`, the engine owns `Box<dyn Renderer>` and calls it directly.
2. **Per-frame `resize()`.** The render_sink calls `r.resize(viewport)` every frame — recreating the Scene + reconfiguring the GPU surface unnecessarily. The `render_commands` trait args (`physical_width, physical_height, dpr`) are **always ignored** by both backends (`webgl_renderer.rs:123-125`, `renderer.rs:347-349`).

**Fix:** MainBackend owns the renderer. No callback.

**Changes:**
1. **`TurRuntime::create_app`** (`runtime/mod.rs:153`): signature becomes `create_app(&self, renderer: Box<dyn Renderer>, viewport: (f64, f64), dpr: f64)`. Stores the renderer in MainBackend.
2. **`MainBackend`** (`backend.rs`): add `renderer: RefCell<Box<dyn Renderer>>`. Delete `render_sink` / `set_render_sink` / `RenderSink` type. In `pump()`:
   - `MainMsg::RenderCommands { commands, image_map }` → `self.renderer.borrow_mut().render_commands(&commands, &image_map)` + `present()`. No viewport field, no `resize()`.
   - `MainMsg::Resized { logical_width, logical_height, dpr }` → `self.renderer.borrow_mut().resize(lw, lh, dpr)` (new variant, see below).
3. **`comm.rs`:** `MainMsg::RenderCommands` drops `viewport` field. Add `MainMsg::Resized { logical_width: u32, logical_height: u32, dpr: f64 }`.
4. **Worker:** after flush, if a `PlatformEvent::Resize` was processed this frame, ship `MainMsg::Resized` (deduped — only when viewport changed, like `CursorChanged`). `ResizeSubsystem` (`resize.rs`) updates the comment (currently documents the old render_sink design at lines 17-21).
5. **`Renderer` trait** (`renderer.rs`): `render_commands` drops `physical_width, physical_height, dpr` args (always ignored — dimensions come from `self`, synced via `resize()`).
6. **`TurApp`:** expose `render_to_pixels()` (delegates to owned renderer, like `main` — `vello_app.rs:154-160`). Delete `set_render_sink`.
7. **Embedders** (`tur-wasm/src/app.rs`, `tur-android/src/app.rs`, `tur-integration-tests/tests/vello/vello_app.rs`):
   - `create_app(Box::new(renderer), (lw, lh), dpr)` — pass renderer directly.
   - Delete the `set_render_sink` closure + the per-frame `resize()` / `render_commands()` / `present()` calls.

**Effect:**
- During steady-state animation: zero `resize()` calls — no per-frame Scene allocation, no GPU surface reconfigure.
- Resize fires exactly when `main` fires it: on `PlatformEvent::Resize` only.
- Simpler embedder API: no callback to install, no viewport plumbing.
- Eliminates one closure allocation + one indirection per frame.

---

## 6. Files touched (summary)

| Phase | Files |
|-------|-------|
| 1 (pipelining) | `core/runtime/backend.rs` (`pump` latest-wins), `tur-wasm/src/lib.rs`, `tur-android/src/app.rs` |
| 2 (MainTree→main) | `core/app/internal.rs`, `core/runtime/backend.rs` |
| 3 (topology diff) | `core/render/main_tree.rs`, `core/app/internal.rs` |
| 4 (image map) | `core/app/context.rs`, `builtin_plugins/image/bridge.rs`, `core/app/comm.rs` |
| 5 (block_on→spawn_local) | `tur-wasm/src/scheduler.rs` (`block_on` → `spawn_local`), `core/scheduler.rs` (trait doc) |
| 6 (own renderer) | `core/runtime/mod.rs` (`create_app` takes `Box<dyn Renderer>`), `core/runtime/backend.rs` (own renderer, delete `render_sink`), `core/app/comm.rs` (add `Resized`, drop `viewport`), `core/render/renderer.rs` (drop dim args from `render_commands`), `lib.rs` (`TurApp` exposes `render_to_pixels`, delete `set_render_sink`), `core/screen/resize.rs` (update comment + ship `Resized`), `tur-wasm/src/app.rs` + `tur-android/src/app.rs` + `tur-integration-tests/tests/vello/vello_app.rs` (pass renderer to `create_app`, delete render_sink wiring) |

## 7. Risks & validation

- **Phase 1 (pipelining):** 1-frame input latency. Verify worker can't get >1 ahead.
- **Phase 5 (block_on→spawn_local):** `spawn_worker`'s closure returns immediately (instead of blocking until `worker_loop` completes). Verify the Web Worker stays alive — `spawn_local` roots the future in the event loop; pending microtasks keep the worker spinning. Verify async completions (`core::async_/completion.rs`) and boa JobExecutor still work (they already use `spawn_local` on the main thread).
- **Phase 6 (own renderer):** the initial viewport is set at renderer construction; the first `PlatformEvent::Resize` (pushed by `create_app`) ships `MainMsg::Resized` — verify the renderer isn't double-resized on startup. The `render_commands` trait signature change (dropping dim args) touches all impls + all call sites. `Box<dyn Renderer>` is `!Send` — verify it stays on main only (MainBackend never sends it to the worker). The embedder loses direct access to the renderer — expose `TurApp::render_to_pixels()` (and any other access the embedder needs) like `main` does.

**Verification workflow (per AGENTS.md):** for each phase, write a failing integration test (red), implement (green), then `cargo test --workspace --test element` + `cargo clippy --workspace -- -D warnings`. Re-profile with Firefox Profiler (enable stackwalk for wasm symbolication) after Phases 1 and 5.

## 8. Out of scope

- **Glyph atlas cache ON.** Considered and explicitly deferred — `main` runs atlas-OFF; keeping it OFF avoids pixel-diff rebaselines. The profiles confirm the encode cost is the same regardless of threading. Future encode optimization is independent.
- **Worker-built Scene (Route A).** Rejected — fragile and unnecessary (vello is immediate-mode).
- **vello_hybrid fork.** Not needed — everything stays on main.
- **Retained/incremental Scene encoding.** vello_hybrid's `Scene` is immediate-mode. Main encode stays full each changed frame.
