# tur

A JavaScript rendering engine built with winit, vello, and boa_engine. Renders SolidJS applications via a custom universal renderer (`@tur/solidjs-renderer`).

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  js/packages/tur-solidjs-demo                       │
│  (SolidJS app, bundled by rspack → defines           │
│   globalThis.startApp())                             │
├─────────────────────────────────────────────────────┤
│  js/packages/tur-solidjs-renderer                    │
│  (solid-js/universal renderer → calls                │
│   globalThis.__tur.*)                                │
└──────────────────────┬──────────────────────────────┘
                       │ JS bridge API
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-engine (unified engine crate)               │
│  ├── core/trait_   (ElementKind, ElementNodeId,       │
│  │                   ElementLayout, ElementRender,     │
│  │                   ElementOnUpdate)                  │
│  ├── core/elements (AnyElement, ElementNode,           │
│  │                   ElementTree with layout+paint)    │
│  ├── core/render   (PaintContext, Renderer,            │
│  │                   ChildLayout, ChildPaint)          │
│  ├── core/bridge   (boa_engine JS bridge, init_bridge) │
│  ├── elements/     (FlexElement, StackElement, etc.    │
│  │                   each with element.rs + render.rs)  │
│  ├── renderer/vello (VelloRenderer, VelloPaintContext) │
│  └── renderer/noop  (NoopRenderer, logs tree stats)    │
└──────────────────────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-wasm                                        │
│  (wasm binary via wasm-pack: winit + boajs + vello)  │
└─────────────────────────────────────────────────────┘
```

### Element types

`Column`, `Row`, `Expanded`, `Stack`, `Positioned`, `SizedBox`, `Container`, `Text`

Flutter-like layout model: flex-based Column/Row with Expanded children, Stack with Positioned children.

### Domain traits

Each element implements three focused traits:

- `ElementOnUpdate` — JS property mutation (`set_prop`)
- `ElementLayout` — two-phase layout (`perform_layout_size`, `perform_layout_position`)
- `ElementRender` — painting and hit testing (`paint`, `hit_test`, `type_name`)

Elements are type-erased via `AnyElement` (private `Erased` trait with blanket impl for all domain traits).

### Data flow

1. JS calls `globalThis.__tur.*` → bridge creates `AnyElement` in `ElementTree`
2. `ElementTree::compute_layout()` runs two-phase layout directly on elements
3. `ElementTree::paint()` walks the tree, calling each element's paint via `PaintContext`
4. `Renderer::render(&mut self, tree: &ElementTree)` drives the frame

## Directory structure

```
libs/
  tur-engine/                # Unified engine crate
    src/
      core/
        trait_/              # Domain traits (ElementLayout, ElementRender, ElementOnUpdate)
        elements/            # AnyElement, ElementNode, ElementTree
        render/              # PaintContext, Renderer, ChildLayout, ChildPaint
        bridge/              # boa_engine JS bridge (init_bridge, TurAppContext)
      elements/              # Concrete elements (flex/, stack/, positioned/, etc.)
        flex/element.rs      # FlexElement struct + ElementOnUpdate
        flex/render.rs       # ElementLayout + ElementRender (layout algorithm)
      renderer/
        vello/               # VelloRenderer (GPU painting)
        noop/                # NoopRenderer (logging)
  tur-shared/                # Shared types (Size, Offset, Constraints, enums, Color)
  tur-wasm/                  # wasm binary (winit + vello + tur-engine)
js/
  packages/
    tur-solidjs-renderer/    # SolidJS universal renderer
    tur-solidjs-demo/        # Demo app (todolist example)
    tur-rspack-plugin/      # Rspack plugin for WASM build + HTML generation
```

## Commands

### Rust (workspace root)

```sh
cargo build --workspace
cargo test --workspace --test element
cargo clippy --workspace -- -D warnings
```

**Before running tests**, prepare JS fixtures (install deps, generate TS types, build JS):

```sh
node scripts/prepare-js-fixtures.cjs
```

### tur-wasm (wasm)

```sh
cd libs/tur-wasm && wasm-pack build --target web
cargo clippy --target wasm32-unknown-unknown --workspace -- -D warnings
```

### tur-rspack-plugin (WASM + HTML)

The `TurRspackPlugin` is used in the demo's rspack config. Building the demo
automatically runs `wasm-pack` and copies WASM artifacts into the output:

```sh
# Build JS bundle (plugin handles wasm-pack + HTML generation)
cd js && pnpm --filter @tur/solidjs-demo build
# Or use the rspack dev server
cd js/packages/tur-solidjs-demo && rspack dev
```

### JS (js/ directory)

```sh
pnpm install
pnpm build            # build all packages
pnpm lint             # oxlint across all packages
```

### Per-package JS builds

```sh
cd js/packages/tur-solidjs-renderer && pnpm build
cd js/packages/tur-solidjs-demo && pnpm build
```

## Conventions

- Rust edition 2024, MSRV 1.85
- JS: TypeScript strict mode, ESNext modules, rspack bundling
- Linting: oxlint with recommended rules
- Layout: Flutter-inspired (Column, Row, Expanded, Stack, Positioned)
- Rendering: vello (GPU vector graphics via wgpu), or noop renderer (logs tree stats)
- JS engine: boa_engine (pure Rust, compiles to wasm32)
- No separate RenderTree — layout and paint happen directly on ElementTree

### Renderer trait

The `Renderer` trait is defined in `tur-engine::core::render`:

```rust
pub trait Renderer {
    fn render(&mut self, tree: &ElementTree);
    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
```

Use `VelloRenderer` for GPU rendering or `NoopRenderer` for debug logging.

## git-end agent

Dispatch `@git-end` to finalize a feature branch. It commits, rebases onto main, pushes, creates/updates a PR, and runs local CI. It reports back: commit hash, PR URL, and CI result (pass or fail with error output). If CI fails, fix the issues and re-dispatch `@git-end`.
