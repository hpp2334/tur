# Phase 3 — MainTree + render-commands wired into flush

**Status:** not started
**Prerequisite:** Phase 1 + Phase 2 complete (CanvasOp, RenderCommand, RecordingCanvas, Canvas notify hooks).
**Goal:** switch the engine's render path from "walk ElementTree directly" to "record → ship commands → main applies + plays back." Single-threaded in this phase; both wasm and native still run on the calling thread. Old direct-render path kept behind a `direct-render` feature for parity testing.

## Background

After Phase 2 we have:
- `RecordingCanvas` that captures per-node boundaries via `notify_node_entry`/`exit` and post-processes into `Vec<RenderCommand::Paint>` (`recording_canvas.rs::into_render_commands`).
- `RenderCommand` enum with `Paint` / `SetChildren` / `Cursor` / `Remove` variants.
- The existing paint walk (`NodeTreeData::paint_element`) already calls the notify hooks, so recording works by swapping `VelloPaintContext` → `RecordingCanvas`.

Phase 3 introduces the long-lived main-side tree (built from commands), a new `Renderer` entry point that renders from it, and wires `flush()` to use the new path.

## Files to add

### `libs/tur-engine/src/core/render/main_tree.rs`

```rust
use std::collections::HashMap;
use crate::core::element::ElementNodeId;
use crate::core::layout::Size;
use crate::core::render::Canvas;
use crate::core::render::command::{CanvasOp, RenderCommand};
use vello_common::kurbo::Affine;

/// Long-lived render tree on main. Updated by applying batches of
/// `RenderCommand` (commit-log style). Walked linearly for playback —
/// no parent-chain traversal (Paint carries absolute transform).
pub struct MainTree {
    nodes: HashMap<ElementNodeId, MainNode>,
    root: Option<ElementNodeId>,
}

struct MainNode {
    /// Last `Paint.transform` received for this id (absolute affine).
    transform: Affine,
    /// Last `Paint.size` received.
    size: Size,
    /// Last `Paint.ops` received (replaces previous — no per-segment
    /// accumulation; the worker already coalesces segments if needed).
    ops: Vec<CanvasOp>,
    /// Last `SetChildren.child_ids` received (for dev-tool queries /
    /// future main-side hit-test — NOT used by playback itself).
    child_ids: Vec<ElementNodeId>,
}

impl MainTree {
    pub fn new() -> Self { ... }

    /// Apply a batch atomically: each command mutates the tree.
    /// `Cursor` is not stored (transient — caller applies immediately).
    pub fn apply_batch(&mut self, batch: Vec<RenderCommand>) { ... }

    /// Walk the tree from root, pushing each node's absolute transform
    /// and playing its ops. Children recursion is implicit: the worker
    /// emits Paint commands in playback order, so a simple linear walk
    /// of nodes.values() in insertion order is NOT correct — we need
    /// the original Paint order.
    ///
    /// Wait — actually, since the worker emits commands in playback
    /// order, main can playback the *batch* itself, not the tree!
    /// See "Design decision" below.
    pub fn playback(&self, canvas: &mut dyn Canvas) { ... }
}
```

### Design decision: playback from batch or from tree?

Two options:

**Option A — playback walks MainTree:**
- Main maintains tree from commands
- Each frame: worker sends batch → main applies → main walks tree to render
- Main needs to remember the "paint order" (which is not the tree's natural order — parent-child interleaving)
- Requires storing an explicit `paint_order: Vec<(ElementNodeId, ops_segment_index)>` or similar

**Option B — playback iterates the batch directly:**
- Main still maintains tree (for dev tools / `SetChildren` queries / `Remove` cleanup)
- But for *rendering*, main just iterates the batch's `Paint` commands in order
- Each `Paint { id, transform, size, ops }` → push transform, play ops, pop
- The clip/opacity layer state persists across Paints automatically (shared canvas/scene)

**Recommendation: Option B.** Simpler, matches how display lists work in browsers. MainTree is just for non-render queries (dev tools, future hit-test). The render path doesn't need a tree at all — it just plays commands linearly.

### `Renderer::render_commands` (extend trait)

```rust
// core/render/renderer.rs
pub trait Renderer {
    fn render(&mut self, ...) { /* existing — behind `direct-render` feature */ }

    /// New primary entry — render from a flat command batch.
    /// Default impl: iterate Paint commands, push transform, play ops.
    /// Concrete renderers (Vello, Noop) override to integrate with their
    /// own canvas/scene setup.
    fn render_commands(&mut self, commands: &[RenderCommand], viewport: Viewport);
}
```

## Files to modify

### `libs/tur-engine/src/core/app/internal.rs` — `flush()`

Today `flush()` ends with:
```rust
if needs_render {
    self.app_context.borrow_mut().render();  // walks NodeTreeData directly
    self.app_context.borrow_mut().renderer.present()?;
}
```

New path (when not under `direct-render` feature):
```rust
if needs_render {
    // 1. Record: walk ElementTree with RecordingCanvas, produce Vec<RenderCommand>.
    let mut recording = RecordingCanvas::new();
    let shell_face = self.app_context.borrow().shell.paint_face();
    {
        let ctx = self.app_context.borrow();
        let tree = ctx.element_tree.borrow();
        let image_map = ctx.image_resource_map.borrow();
        tree.paint(&mut recording, ctx.focus_manager.borrow().focused(), &image_map, shell_face);
    }
    let commands = recording.into_render_commands();

    // 2. (Optional) Maintain MainTree for dev tools: main_tree.apply_batch(commands.clone())

    // 3. Render: pass commands to renderer.
    let viewport = self.app_context.borrow().screen.viewport();
    self.app_context.borrow_mut().renderer.render_commands(&commands, viewport);
    self.app_context.borrow_mut().renderer.present()?;
}
```

### Cursor + SetChildren + Remove in the record pass

Phase 2 produces only `Paint` commands. Phase 3 extends the record pass to emit the other variants:

- **Cursor** — read from `Shell::CursorSink` after the paint walk (the recording already accumulates claims via `paint_ctx.set_cursor`); emit `Cursor { cursor }` if changed from previous frame.
- **SetChildren** — walk the ElementTree topology; emit `SetChildren { id, child_ids }` for each node whose topology changed since last frame. Worker tracks `last_topology: HashMap<ElementNodeId, Vec<ElementNodeId>>`.
- **Remove** — track destroyed ids (drain `pending_destroy`-style); emit `Remove { id }` per destroyed node.

For Phase 3 v1, emit `SetChildren` for every node every frame (full sync) — diff optimization deferred. Cursor dedup against last frame is required (decided earlier).

### `libs/tur-engine/Cargo.toml`

```toml
[features]
default = []
direct-render = []   # opt-in: use old `Renderer::render(&NodeTreeData)` path
```

## Verification

1. **Parity test** — with `direct-render` feature ON, the existing snapshot tests must still pass. With feature OFF, the same tests should produce identical pixels. Add a test mode that runs both paths and asserts pixel equality.
2. `cargo test --workspace --test element --test vello` — all 170 + 8 tests pass under both feature configurations.
3. `cargo build --workspace` and `cargo build --workspace --features tur-engine/direct-render` both clean.
4. `cargo clippy --workspace -- -D warnings` clean under both.
5. `cargo check --target wasm32-unknown-unknown -p tur-engine` clean.

## Risks

- **Pixel parity** — the new path must produce identical output to the old. Vello's Scene build happens the same way (via VelloPaintContext); only the order of calls changes (linear batch vs recursive walk). If clip/opacity layers work as expected, parity should hold. Verify with the existing vello snapshot tests.
- **Cursor claim timing** — today the shell applies cursor *after* paint walk. New path: cursor is in the command batch, applied at playback. Slight timing shift but semantically equivalent.
- **Performance** — the new path adds one extra step (record + post-process). Should be negligible (record is just method-call dispatches into a Vec).

## Out of scope for Phase 3

- Multi-threading — still single-threaded.
- WorkerMsg/MainMsg channels — that's Phase 4.
- Event bus migration — that's Phase 5.
- API breakage — `TurApp` public API unchanged in Phase 3.

## Estimated scope

- ~200 lines new (MainTree, Renderer::render_commands, record-pass extensions for Cursor/SetChildren/Remove)
- ~30 lines modified (flush loop, Cargo.toml)
- Parity test infrastructure
- Single PR, medium-complexity review
