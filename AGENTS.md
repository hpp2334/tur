# tur

A JavaScript rendering engine built with winit, vello, and boa_engine. Renders React applications via a custom reconciler (`@tur/react-renderer`).

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  js/packages/tur-demo (playground web app)             │
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
│  ├── core/trait_   (ElementKind, NodeId,             │
│  │                   ElementLayout, ElementRender,     │
│  │                   ElementOnUpdate, ElementSubscribe)│
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

### Animation model (Flutter-aligned)

The engine exposes only the animation **primitives**; the `Animated*` widget family is composed in JS (`@tur/edgy`), not implemented as native elements.

- **`Curve`** (`tur-shared::curve`) — a time-remap `f64 → f64` (Flutter `Curve`): `Linear`/`EaseIn`/`EaseOut`/`EaseInOut`. Parsed from JS strings like `"easeInOut"`.
- **`Tween<T>`** (`tur-shared::tween`) — a value range `{begin, end}` with `lerp(t) → T` (Flutter `Tween<T>`). `NumTween` for `f64`, `ColorTween` for component-wise `Color` interpolation via `Color::lerp`. Exposed in JS as `Tween({begin, end})` / `ColorTween({begin, end})` with mutable `begin`/`end` and `lerp`/`transform` methods.
- **Explicit animation**: `createAnimationController({duration, curve, repeat, onTick, onEnd})` drives a source atom via `onTick`; pair with `Tween.lerp(t)` in a `derive()` for explicit, controller-driven interpolation (continuous loops, transport controls). See the `complex-animation` case.
- **Implicit animation** (JS, in `@tur/edgy`): `AnimatedContainer` / `AnimatedOpacity` / `AnimatedPositioned` wrap their plain siblings (`Container` / `Opacity` / `Positioned`). Each animatable prop is a `Tween` channel displayed as `tween.lerp(progress)`; one shared `progress` source is driven by a single `AnimationController`'s `onTick`. `ReadableSubscribe` watches the reactive targets — on change, `onUpdate$` rebases each channel's `begin` to its currently-displayed value, sets `end` to the new target, and restarts the controller (Flutter's `ImplicitlyAnimatedWidget` retarget). Static props pass through. See the `implicit-animations` case.

### Domain traits

Each element implements these focused traits:

- `ElementOnUpdate` — JS property mutation (`set_prop`)
- `ElementLayout` — layout (`perform_layout`: measure children, compute own size, assign child offsets in one pass)
- `ElementRender` — painting and hit testing (`paint`, `hit_test`, `type_name`)
- `ElementSubscribe` — declares which reactive atoms the node depends on (`subscribe`), so a reactive flush can mark it dirty for re-layout. Runs as an explicit phase after `perform_layout` for dirty nodes.

Elements are type-erased via `AnyElement` (private `Erased` trait with blanket impl for all domain traits).

### Data flow

1. JS calls `globalThis.__tur.*` → bridge creates `AnyElement` in `ElementTree`
2. `ElementTree::compute_layout()` lays out dirty nodes: each node runs `perform_layout` (resolving `Val<T>` props untracked) then `subscribe` (explicitly re-declaring its reactive deps into the store's atom→subscriber index)
3. When an atom changes, a reactive flush maps stale atoms → subscribed nodes via `dirty_subscribers` → `mark_dirty` (propagates to ancestors) → next layout re-resolves values
4. `ElementTree::paint()` walks the tree, calling each element's paint via `PaintContext`
5. `Renderer::render(&mut self, tree: &ElementTree)` drives the frame

## Directory structure

```
libs/
  tur-engine/                # Unified engine crate
    src/
      core/
        trait_/              # Domain traits (ElementLayout, ElementRender, ElementOnUpdate, ElementSubscribe)
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
    tur-edgy/                # Flutter-like component wrappers + reactivity (Column, Row, Match, Dynamic, ...)
    tur-demo/                # Playground: thin browser wrapper (loads wasm + impl bundle)
    tur-demo-impl/           # Playground UI built with @tur/edgy (Sidebar/Editor/Viewer)
    tur-test-cases/          # Test cases (edgy-cases/, ~60 cases)
    tur-react-renderer/      # (legacy) React reconciler, superseded by @tur/edgy
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

### tur-demo (playground)

The playground is a React-DOM web app. Building it automatically runs `wasm-pack`, builds test cases, copies WASM assets + compiled cases + workspace deps into the output:

```sh
cd js && pnpm build
cd js/packages/tur-demo && rspack build
# Or use the rspack dev server
cd js/packages/tur-demo && rspack dev
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
- When developing, especially writing demo cases, if an engine-level issue is found, investigate and plan to fix it in the engine rather than working around it in the demo case itself.

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

## Debugging the playground (main agent + image-reader)

The whole playground (sidebar + editor + viewer) renders to a single `<canvas>` — tur renders its own UI. The main agent drives the browser directly via Playwright MCP tools and reads screenshots with the **image-reader** subagent (Task tool, `image-reader` type).

### Start the dev server

```sh
node scripts/prepare-js-fixtures.cjs    # build JS fixtures once
cd js/packages/tur-demo && TUR_TUNNEL=1 rspack dev
# → http://localhost:8080/ (must be HTTP: Playwright MCP rejects the self-signed HTTPS cert)
```

### Drive the canvas

1. `playwright_browser_navigate` → `http://localhost:8080/`.
2. `playwright_browser_evaluate` → read `JSON.parse(globalThis.turDevTool.elementTree())` for exact element rects. The root node carries `{ id, name, label, props, layout:{relative,absolute,width,height,extra?}, queryKey?, children:[{id}, ...] }`; drill into a child via `JSON.parse(globalThis.turDevTool.getElement(childId))`. Hit-testing is pixel-precise: sidebar items are left-aligned at `x=0` and only as wide as their label (56–163px), so click at a small `x` (e.g. 30), not the column center.
3. Click/type by dispatching events on the canvas, e.g. `canvas.dispatchEvent(new MouseEvent('mousedown', { clientX, clientY }))` + matching `mouseup`. Keyboard: dispatch `KeyboardEvent` on the focused element (canvas or the hidden `<textarea>` when an `EditableText` has focus).
4. Re-read `turDevTool.elementTree()` / `getElement(id)` or take a screenshot to confirm the result.

### Verify visually with image-reader

`turDevTool.elementTree()` can report a correct tree while the canvas is visually blank or wrong (e.g. zero-width / transparent elements). After any rendering change, capture a screenshot with `playwright_browser_take_screenshot` and pass the file path to the **image-reader** subagent (Task tool, `image-reader` type) with a focused PASS/FAIL question. Only visual verification catches blank canvases, wrong colors, missing text, or stretched elements. For color checks, prefer ground truth — sample actual canvas pixels via `getImageData` rather than eyeballing, since color perception is unreliable.

### Stop the dev server after verification

Once visual verification is done, **kill the dev server** — free port 8080 with `lsof -ti:8080 | xargs kill` (or `pkill -f "rspack dev"`). Do not leave it running — it holds port 8080 and rebuilds wasm on every watch cycle.

### Clean up screenshots after verification

If a screenshot was saved with a bare `filename`, it lands at the workspace root and shows up as an untracked file. After every visual-verification round, remove stray workspace-root PNGs so the working tree stays clean:

```sh
rm -f *.png  # only stray workspace-root screenshots; safe since no PNGs are tracked at root
```

Verify with `git status` — only the intended source changes should remain. Never commit a screenshot.

