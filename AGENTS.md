# tur

A JavaScript rendering engine built with winit, vello, and boa_engine. Renders React applications via a custom reconciler (`@tur/react-renderer`).

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  js/packages/tur-react-demo (playground web app)     │
│  React-DOM web app with:                             │
│  - Case selector (sidebar)                           │
│  - Code editor (CodeMirror 6)                        │
│  - Tur viewer (embedded WASM canvas)                 │
│  - Browser-side bundling via @rspack/browser          │
├─────────────────────────────────────────────────────┤
│  js/packages/tur-test-cases                          │
│  ~60 React test cases in react-cases/                 │
│  Each case calls renderRoot(Component)                │
├─────────────────────────────────────────────────────┤
│  js/packages/tur-react                                │
│  React component wrappers (Column, Row, Container…)  │
├─────────────────────────────────────────────────────┤
│  js/packages/tur-react-renderer                       │
│  Custom React reconciler → globalThis.__tur.*         │
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
│  (wasm binary via wasm-pack: boajs + vello)           │
│  TurWasmApp::create() — full viewport                 │
│  TurWasmApp::create_in(id) — embed in container       │
│  clear_and_run_js() — clear tree + evaluate new JS    │
└─────────────────────────────────────────────────────┘
```

### Element types

`Column`, `Row`, `Expanded`, `Stack`, `Positioned`, `SizedBox`, `Container`, `Text`, `Input`, `PointerInteract`, `Focusable`, `Paragraph`, `Image`, `Svg`

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
  tur-wasm/                  # wasm binary (boa_engine + vello + tur-engine)
js/
  packages/
    tur-react/               # React component wrappers
    tur-react-renderer/      # Custom React reconciler
    tur-react-demo/          # Playground web app (React-DOM + CodeMirror + tur viewer)
    tur-test-cases/          # Test cases (react-cases/ with ~60 cases)
    playground-for-agent/    # Playwright integration tests
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

### tur-react-demo (playground)

The playground is a React-DOM web app. Building it automatically runs `wasm-pack`, builds test cases, copies WASM assets + compiled cases + workspace deps into the output:

```sh
cd js && pnpm build
cd js/packages/tur-react-demo && rspack build
# Or use the rspack dev server
cd js/packages/tur-react-demo && rspack dev
```

Requires COOP/COEP headers for `SharedArrayBuffer` (configured in devServer).

### JS (js/ directory)

```sh
pnpm install
pnpm build            # build all packages
pnpm lint             # biome lint across all packages
```

### Per-package JS builds

```sh
cd js/packages/tur-react-renderer && pnpm build
cd js/packages/tur-react && pnpm build
cd js/packages/tur-test-cases && pnpm build
```

## Conventions

- Rust edition 2024, MSRV 1.85
- JS: TypeScript strict mode, ESNext modules, rspack bundling
- Linting: biome
- Layout: Flutter-inspired (Column, Row, Expanded, Stack, Positioned). The layout model follows Flutter's flex layout — Column/Row are flex containers, Expanded fills remaining space, Container with explicit width/height constrains to those dimensions. Default cross-axis alignment for both Column and Row is `Center` (matching Flutter's behavior).
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

## Debugging with playground-for-agent

Launches Chromium with WebGPU, loads the playground via Playwright, runs an interactive todolist scenario (add/toggle/delete tasks) and a counter live-editing scenario, and screenshots each step. Screenshots saved to `js/packages/playground-for-agent/test-results/`.

### Always verify with image-reader after running

After running the playground tests, you MUST use `@image-reader` (Task tool with `image-reader` subagent) to inspect the screenshots and verify the actual rendering. Layout assertions check element positions but `@image-reader` reveals what the user actually sees (colors, spacing, text rendering, missing content, blank canvases, stretched elements, etc.).

Tests can pass (elements exist in layout tree, clicks register) while the canvas is visually blank or broken. Only visual verification catches these issues.

Example:

```
@image-reader js/packages/playground-for-agent/test-results/01-initial.png
```

Or verify all screenshots at once by passing all file paths to a single `@image-reader` task.

### Dev mode (local dist)

```sh
cd js && pnpm build
cd js/packages/tur-react-demo && rspack build
cd js/packages/playground-for-agent && pnpm start
```

### Prod mode (deployed URL)

```sh
DEPLOY_URL=https://tur-react-demo.pages.dev pnpm start:prod
```


