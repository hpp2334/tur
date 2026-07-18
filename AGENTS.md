# tur

A JavaScript rendering engine built with winit, vello-hybrid, and boa_engine. Renders React applications via a custom reconciler (`@tur/react-renderer`).

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
│  ├── core/capability (Capability trait, Capabilities,  │
│  │                   CapabilityDecls — type-keyed      │
│  │                   service registry consumed by      │
│  │                   bridge fns, handlers, plugins)    │
│  ├── core/bridge   (boa_engine JS bridge, init_bridge) │
│  ├── core/subsystem (Subsystem trait + flush hook)     │
│  ├── core/text     (TextLayoutData, FontManager —      │
│  │                   paint/layout contract types only) │
│  ├── elements/     (FlexElement, StackElement, etc.    │
│  │                   each with element.rs + render.rs)  │
│  ├── renderer/vello (VelloRenderer, VelloPaintContext) │
│  └── renderer/noop  (NoopRenderer, logs tree stats)    │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-animation (standalone crate)                 │
│  Registered via TurAnimationPlugin. Owns               │
│  AnimationManager + Clock (ticks on each flush via     │
│  the Subsystem hook). Exposes builtin:tur/animation     │
│  (combined native+JS module: Opacity, Transform,        │
│  createAnimationController + AnimatedContainer/Opacity/ │
│  Positioned, Tween, ColorTween) + internal hidden       │
│  tur:animation/native (ctx-bound fns only).             │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-text (text feature library, NOT a plugin)    │
│  Installed into builtin:tur/std by TurStdPlugin via    │
│  install_text_feature(ctx) → Vec<FnEntry>. Owns:       │
│  TextElement, EditableTextElement,                     │
│  TextEditingController, UndoController,                │
│  EnsureCaretVisibleHandler (post-event), and           │
│  extract_layout_data() (bridge: JS → TextLayoutData).  │
│  Engine keeps TextLayoutData/FontManager as paint       │
│  contract; Canvas::fill_text_layout does the drawing.   │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  Capability crates (3-crate split per domain):         │
│  ├── tur-clipboard-capability (Clipboard +             │
│  │   ClipboardBackend trait + builtin:tur/clipboard)   │
│  ├── tur-clipboard-wasm     (WasmClipboard backend)    │
│  ├── tur-clipboard-native   (NativeClipboard via       │
│  │   arboard)                                          │
│  ├── tur-net-capability (Http + HttpBackend trait +    │
│  │   builtin:tur/net)                                  │
│  ├── tur-net-wasm           (WasmHttp via reqwest-wasm)│
│  └── tur-net-native         (NativeHttp via reqwest)   │
│  Embedders register backends via                      │
│    TurEngineBuilder::capability(Clipboard::new(backend))│
│    TurEngineBuilder::capability(Http::new(backend))    │
│  Engine-internal capabilities (e.g. CursorCap for      │
│  CursorBackend) live in tur-engine/stdlib/platform.rs.│
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-wasm                                        │
│  (wasm binary via wasm-pack: boajs + vello-hybrid)     │
│  TurWasmApp::create() — full viewport                 │
│  TurWasmApp::create_in(id) — embed in container       │
│  clear_and_run_js() — clear tree + evaluate new JS    │
│  Composes: Clipboard::new(WasmClipboard),             │
│            Http::new(WasmHttp),                        │
│            CursorCap::new(WasmCursor) + plugins.       │
│            TurStdPlugin → TurAnimationPlugin →         │
│            TurClipboardPlugin → TurNetPlugin.          │
└─────────────────────────────────────────────────────┘
```

### Capability registry

Embedders register swappable backends (clipboard, http, cursor) on the engine builder:

```rust
TurEngine::builder()
    .capability(Clipboard::new(WasmClipboard))   // tur-clipboard-wasm
    .capability(Http::new(WasmHttp))             // tur-net-wasm
    .capability(CursorCap::new(WasmCursor))      // engine-internal
    .plugin(TurStdPlugin)
    .plugin(TurAnimationPlugin)                  // tur-animation (after TurStdPlugin)
    .plugin(TurClipboardPlugin)                  // requires: Clipboard
    .plugin(TurNetPlugin)                        // Http optional (skips builtin:tur/net if absent)
    .build()
```

- `Capability: Any + Clone + 'static` — marker trait, implemented explicitly per
  newtype (`Clipboard`, `Http`, `CursorCap`).
- `Plugin::requires(&mut CapabilityDecls)` — declare hard deps; the builder
  validates them BEFORE any plugin's `register` runs, so missing capabilities
  fail fast at `build()` with a clear error.
- `Capabilities::of::<C>()` / `require::<C>()` — deferred lookup at JS call
  time (bridge fns) or event dispatch time (handlers via
  `HandlerContext.capabilities`).
- Convention: capability newtypes use base names (`Clipboard`, `Http`);
  backend traits use `*Backend` suffix (`ClipboardBackend`, `HttpBackend`,
  `CursorBackend`). `CursorCap` is the lone exception because `tur_shared::Cursor`
  already names the cursor-kind enum.


### Element types

`Column`, `Row`, `Expanded`, `Stack`, `Positioned`, `SizedBox`, `Container`, `PointerInteract`, `Focusable`, `Image`, `Svg` (tur-engine) · `Text`, `Input`, `Paragraph` (tur-text) · `Opacity`, `Transform` (tur-animation)

Flutter-like layout model: flex-based Column/Row with Expanded children, Stack with Positioned children.

### Animation model (Flutter-aligned)

Animation lives entirely in the standalone `tur-animation` crate (registered via `TurAnimationPlugin`). The engine core exposes only the `Subsystem` flush hook + `Clock` accessor — no animation code is in `tur-engine`.

- **`Subsystem` trait** (`tur-engine::core::subsystem`) — `fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) -> SubsystemOutcome`. Runs once per `flush()` call, in registration order. `AnimationSubsystem` owns `AnimationManager` + the engine `Clock` and ticks the manager each flush.
- **`Curve`** (`tur-shared::curve`) — a time-remap `f64 → f64` (Flutter `Curve`): `Linear`/`EaseIn`/`EaseOut`/`EaseInOut`. Parsed from JS strings like `"easeInOut"`.
- **`Tween<T>`** (`tur-shared::tween`) — a value range `{begin, end}` with `lerp(t) → T` (Flutter `Tween<T>`). `NumTween` for `f64`, `ColorTween` for component-wise `Color` interpolation via `Color::lerp`. Exposed in JS as `Tween({begin, end})` / `ColorTween({begin, end})` with mutable `begin`/`end` and `lerp`/`transform` methods.
- **Effect elements**: `Opacity` (alpha-mask a child) and `Transform` (rotate/scale/translate). Registered by `tur-animation` under `builtin:tur/animation`.
- **Explicit animation**: `createAnimationController({duration, curve, repeat, onTick, onEnd})` drives a source atom via `onTick`; pair with `Tween.lerp(t)` in a `derive()` for explicit, controller-driven interpolation (continuous loops, transport controls). See the `complex-animation` case.
- **Implicit animation** (JS, in `tur-animation`'s `js/index.js`): `AnimatedContainer` / `AnimatedOpacity` / `AnimatedPositioned` wrap their plain siblings (`Container` / `Opacity` / `Positioned`). Each animatable prop is a `Tween` channel displayed as `tween.lerp(progress)`; one shared `progress` source is driven by a single `AnimationController`'s `onTick`. `ReadableSubscribe` watches the reactive targets — on change, `onUpdate$` rebases each channel's `begin` to its currently-displayed value, sets `end` to the new target, and restarts the controller (Flutter's `ImplicitlyAnimatedWidget` retarget). Static props pass through. See the `implicit-animations` case.

`tur-animation` registers ONE combined consumer-facing module `builtin:tur/animation` (JS source loaded via `include_str!` + `register_js_module`) that re-exports native fns (`Opacity`, `Transform`, `createAnimationController`) from the hidden `tur:animation/native` module and defines the JS widgets on top.

### Text model

Text logic lives in the standalone `libs/tur-text` crate — **not** a plugin. It is installed into `builtin:tur/std` by `TurStdPlugin` via `install_text_feature(ctx: &mut PluginContext) -> Result<Vec<FnEntry>, TurError>`. The returned `FnEntry`s are merged into `std_fns` before `register_module("builtin:tur/std", ...)`, so `Text` / `Input` / `createTextEditingController` / `createUndoController` ship as part of the std module from JS's perspective.

- **Engine contract types** (kept in `tur-engine::core::text::text_layout` + `core::fonts`): `TextLayoutData`, `LineInfo`, `LineGlyphStop`, `TextRunData`, `TextGlyph`, `FontManager`, `FontLoader`. The engine's `Canvas::fill_text_layout(&TextLayoutData)` does the actual drawing; tur-text only produces these structs.
- **`extract_layout_data(props) -> TextLayoutData`** (tur-text, in `src/text_layout.rs`): bridge helper that turns JS-side text props into the engine's `TextLayoutData` used by layout + paint.
- **Elements** (`tur-text::elements`): `TextElement` (static text), `EditableTextElement` (cursor + selection + IME + paste), `ParagraphElement`.
- **Controllers** (`tur-text::controller`): `TextEditingController` (registered class — `register_class`), `UndoController`, plus `SpanData` + event types.
- **Post-event caret visibility** (`tur-text::handlers`): `EnsureCaretVisibleHandler` runs after keyboard/IME/paste handlers (in registration order) and scrolls the focused editable's `ScrollView` to keep the caret in view. The engine's `keyboard.rs` / `ime.rs` no longer call caret-scroll directly.
- **Paste dispatch** (engine → tur-text): the engine's `ClipboardPasteAppHandler` (in `tur-engine::core::handlers`, registered by `TurStdPlugin`) forwards the embedder's `PlatformEvent::ClipboardPaste` as `AppEvent::ClipboardPaste` on the engine-internal event bus. tur-text's `ClipboardPasteHandler` (in `tur-text::handlers`) consumes the AppEvent, looks up the focused `EditableTextElement`, and inserts the text (replacing any selection, or at the caret). No per-element trait is needed: paste is a single-consumer, stateless op. The engine stays free of any text-element knowledge.

JS surface is unchanged — `builtin:tur/std` still exports Text/Input/etc. No `.d.ts` split, no new JS package.

### Domain traits

Each element implements these focused traits:

- `ElementOnUpdate` — JS property mutation (`set_prop`)
- `ElementLayout` — layout (`perform_layout`: measure children, compute own size, assign child offsets in one pass)
- `ElementRender` — painting and hit testing (`paint`, `hit_test`, `type_name`)
- `ElementSubscribe` — declares which reactive atoms the node depends on (`subscribe`), so a reactive flush can mark it dirty for re-layout. Runs as an explicit phase after `perform_layout` for dirty nodes.

Elements are type-erased via `AnyElement` (private `Erased` trait with blanket impl for all domain traits). Paste is **not** an element trait — it flows through `AppEvent::ClipboardPaste` + tur-text's `ClipboardPasteHandler` (see [Text model](#text-model)).

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
        capability/          # Capability trait, Capabilities view, CapabilityDecls
        bridge/              # boa_engine JS bridge (init_bridge, TurAppContext)
        plugin.rs            # Plugin trait (register + requires) + PluginContext
        subsystem.rs         # Subsystem trait + SubsystemFlushContext (per-flush hooks)
        text/                # TextLayoutData + LineInfo + TextRunData (paint/layout
                             #   contract types only — tur-text produces them)
        fonts.rs             # FontManager + FontLoader (used by Canvas::fill_text_layout)
      elements/              # Concrete elements (flex/, stack/, positioned/, etc.)
        flex/element.rs      # FlexElement struct + ElementOnUpdate
        flex/render.rs       # ElementLayout + ElementRender (layout algorithm)
      renderer/
        vello/               # VelloRenderer (GPU painting)
        noop/                # NoopRenderer (logging)
      stdlib/platform.rs     # CursorBackend trait + CursorCap capability (engine-internal)
  tur-shared/                # Shared types (Size, Offset, Constraints, enums, Color)
  tur-animation/             # Animation subsystem (manager/controller/event + Opacity/Transform
                             #   effects + JS widgets) — registered via TurAnimationPlugin, exposes
                             #   `builtin:tur/animation` (combined native+JS module) + internal
                             #   `tur:animation/native` (ctx-bound fns only)
  tur-text/                  # Text feature library (TextElement, EditableTextElement,
                             #   ParagraphElement, controllers, EnsureCaretVisibleHandler,
                             #   extract_layout_data) — NOT a plugin; installed into
                             #   builtin:tur/std by TurStdPlugin via install_text_feature()
  tur-clipboard-capability/  # Clipboard trait + Clipboard cap + builtin:tur/clipboard + handlers
  tur-clipboard-wasm/        # WasmClipboard (navigator.clipboard) backend
  tur-clipboard-native/      # NativeClipboard (arboard) backend
  tur-net-capability/        # HttpBackend trait + Http cap + builtin:tur/net
  tur-net-wasm/              # WasmHttp (reqwest-wasm) backend
  tur-net-native/            # NativeHttp (reqwest) backend
  tur-wasm/                  # wasm binary (boa_engine + vello-hybrid + tur-engine)
js/
  packages/
    tur-animation/            # Ambient TS types for `builtin:tur/animation` (runtime provided by tur-animation crate)
    tur-demo/                # Playground: thin browser wrapper (loads wasm + impl bundle)
    tur-demo-impl/           # Playground UI built with builtin:tur/animation + builtin:tur/std (Sidebar/Editor/Viewer)
    tur-test-cases/          # Test cases (cases/, ~60 cases)
    tur-react-renderer/      # (legacy) React reconciler, superseded by builtin:tur/std
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

- Rust edition 2024, MSRV 1.91
- JS: TypeScript strict mode, ESNext modules, rspack bundling
- Linting: biome
- Layout: Flutter-inspired (Column, Row, Expanded, Stack, Positioned). The layout model follows Flutter's flex layout — Column/Row are flex containers, Expanded fills remaining space, Container with explicit width/height constrains to those dimensions. Default cross-axis alignment for both Column and Row is `Center` (matching Flutter's behavior).
- Rendering: vello-hybrid (hybrid CPU/GPU sparse-strips vector graphics). Two backends: **WebGL2** (`WebGlVelloRenderer`, used by `tur-wasm` — native browser WebGL2, no wgpu dependency, ~3MB smaller binary) and **wgpu** (`VelloRenderer`, used by native integration tests — Vulkan/Metal/DX12/WebGPU). The `renderer/vello` module keeps the historical name. Shared `VelloPaintContext` + `scene_paint` helpers paint the element tree into a vello-hybrid `Scene`; each backend wraps it with its own renderer + `Renderer` trait impl. Backend selection is via tur-engine features: `wgpu-backend` (default, native) vs `webgl` (wasm). Also a noop renderer (logs tree stats).
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

## Invoking the git-end subagent

When the user asks to commit/push/PR (e.g. `@git-end`, "commit and push", "open a PR"), dispatch the **git-end** subagent via the Task tool with `subagent_type: "git-end"` — but **do NOT pass any prompt**. The agent is hard-coded to ignore prompt contents and follow only its own workflow (rebase → commit → push → create/update PR → run local CI). It derives the commit message and PR title/body directly from `git diff` and `git diff main...HEAD --stat`, so a prompt is at best redundant and at worst misleading.

Concretely:
- Pass an empty/minimal `prompt` (e.g. the empty string or a single space — the field is required by the tool schema, but the agent discards it).
- Do **not** pre-stage files, write the commit message, draft the PR body, or summarize "what we did" in the prompt — git-end inspects the tree itself.
- The agent's full workflow lives in `.opencode/agents/git-end.md`.


