# tur

A JavaScript rendering engine built with winit, vello-hybrid, and boa_engine. JS calls into the engine via the `tur:std` / `tur:animation` / `tur:clipboard` / `tur:net` modules registered by engine plugins.

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
│  js/packages/tur-demo-impl                            │
│  Playground UI built with tur:animation +    │
│  tur:std (Sidebar/Editor/Viewer)              │
├─────────────────────────────────────────────────────┤
│  js/packages/tur-test-cases                          │
│  ~60 test cases in cases/ — each calls into           │
│  tur:std directly                            │
└──────────────────────┬──────────────────────────────┘
                       │ JS bridge API
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-engine (unified engine crate)               │
│  ├── core/         (engine infrastructure — NO         │
│  │                  dependency on builtin_plugins/*)   │
│  │   ├── app/      (TurAppInternal + FrameOutcome +    │
│  │   │             AppEvent/AppEventQueue + render()   │
│  │   │             mount + RootView/RootElement        │
│  │   │             generic-root wrapper)               │
│  │   ├── elements/ (AnyElement, ElementObject,         │
│  │   │             ElementTree with layout+paint)      │
│  │   ├── render/   (PaintContext, Renderer,            │
│  │   │             ElementRender trait + brush/         │
│  │   │             Color/Brush/GradientStop + JS        │
│  │   │             bindings)                            │
│  │   ├── layout/   (ElementLayout, ElementSubscribe,   │
│  │   │             LayoutContext, primitives)          │
│  │   ├── capability/ (Capability trait, Capabilities,  │
│  │   │             CapabilityDecls — type-keyed        │
│  │   │             service registry)                   │
│  │   ├── js_runtime/ (boa plumbing: TurJsContext,      │
│  │   │             JsProps, FnEntry, module_loader,    │
│  │   │             opaque, js_value)                   │
│  │   ├── dev/      (turDevTool bridge)                 │
│  │   ├── edgy/     (reactive substrate: Store/Source/  │
│  │   │             Derived/MutationHandle + mutation   │
│  │   │             queue + source/derive/mutate/get/   │
│  │   │             set/view JS bridge — the engine's   │
│  │   │             own tur:core)               │
│  │   ├── focus/    (FocusManager + Focusable trait +   │
│  │   │             BlurEvent/FocusEvent/FocusChange)   │
│  │   ├── screen/   (Screen + viewportSize$ source +    │
│  │   │             ResizeSubsystem)                    │
│  │   ├── platform/ (Cursor/CursorBackend/CursorCap +   │
│  │   │             PlatformEvent/PointerInput/Ime +    │
│  │   │             key_event: KeyEvent/Modifiers/      │
│  │   │             KeydownEvent/KeyupEvent)            │
│  │   ├── subsystem.rs (Subsystem trait + flush hook)   │
│  │   ├── text/     (TextLayoutData, FontManager —      │
│  │   │             paint/layout contract types only)   │
│  │   ├── image_resource.rs (ImageResourceId,           │
│  │   │             ImageResourceMap, ImageResource —   │
│  │   │             paint/layout contract types only)   │
│  │   └── plugin.rs (Plugin trait + PluginContext)      │
│  ├── builtin_plugins/ (feature bundles — each exposes  │
│  │                      one pub install_xxx(ctx))      │
│  │   ├── std.rs    (TurStdPlugin — the orchestrator    │
│  │   │             that calls every install_xxx and    │
│  │   │             merges FnEntry into tur:std)│
│  │   ├── console.rs (global console.log/warn/error/    │
│  │   │               info/debug)                       │
│  │   ├── control_flow/ (Condition/Switch/Each/Fragment)│
│  │   ├── focus/     (Focusable widget — manager is in  │
│  │   │               core::focus)                      │
│  │   ├── gesture/   (MouseRegion + PointerInteract +   │
│  │   │               GestureSubsystem + PointerSubsystem)│
│  │   ├── input/     (KeyboardSubsystem + ImeSubsystem —│
│  │   │               event types are in core::platform)│
│  │   ├── layout/    (Column/Row/Expanded/Stack/        │
│  │   │               Positioned/Container/SizedBox +   │
│  │   │               layout enums)                     │
│  │   ├── lifecycle/ (lifecycleView)                    │
│  │   ├── text/      (TextElement, EditableTextElement, │
│  │   │               ParagraphElement, controllers,    │
│  │   │               ClipboardPasteSubsystem,          │
│  │   │               CaretVisibilitySubsystem)         │
│  │   ├── image/     (ImageElement + PNG/JPEG/SVG       │
│  │   │               decoders)                         │
│  │   ├── scroll/    (ScrollView, Scrollbar,            │
│  │   │               ScrollController, ScrollSubsystem)│
│  │   └── lazy_container/ (LazyList + LazyListController)│
│  ├── renderer/vello (VelloRenderer, VelloPaintContext) │
│  └── renderer/noop  (NoopRenderer, logs tree stats)    │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-animation (standalone crate)                 │
│  Registered via TurAnimationPlugin. Owns               │
│  AnimationManager + Clock (ticks on each flush via     │
│  the Subsystem hook). Exposes tur:animation     │
│  (combined native+JS module: Opacity, Transform,        │
│  createAnimationController + AnimatedContainer/Opacity/ │
│  Positioned, Tween, ColorTween) + internal hidden       │
│  tur:animation/native (ctx-bound fns only).             │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  Capability surfaces:                                  │
│  ┌─ Inlined into tur-engine (plugin + contract types): │
│  │  • Clipboard   — `builtin_plugins::clipboard`       │
│  │    (ClipboardBackend trait + Clipboard cap +        │
│  │    tur:clipboard + engine-internal          │
│  │    subsystems + event payloads)                     │
│  │  • Cursor      — `core::platform::cursor`           │
│  │    (CursorBackend trait + CursorCap + Cursor enum;  │
│  │    no JS bridge — engine-internal only)             │
│  └─ External capability crates (split per domain):     │
│     ├── tur-net-capability (Http + HttpBackend trait + │
│     │   tur:net)                               │
│     ├── tur-net-wasm           (WasmHttp via reqwest-wasm)│
│     └── tur-net-native         (NativeHttp via reqwest)│
│  Backend crates for the inlined Clipboard cap:         │
│     ├── tur-clipboard-wasm  (WasmClipboard — re-exports│
│     │                       Clipboard/ClipboardBackend/│
│     │                       TurClipboardPlugin from    │
│     │                       tur_engine)                │
│     └── tur-clipboard-native (NativeClipboard via      │
│                                arboard, same re-exports)│
│  Embedders register backends via                       │
│    TurEngineBuilder::capability(Clipboard::new(backend))│
│    TurEngineBuilder::capability(Http::new(backend))    │
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
│            (TurStdPlugin + TurClipboardPlugin re-      │
│            exported from tur_engine crate root — used  │
│            to live in separate tur-std /               │
│            tur-clipboard-capability crates.)           │
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
    .plugin(TurNetPlugin)                        // Http optional (skips tur:net if absent)
    .build()
```

- `Capability: Any + Clone + 'static` — marker trait, implemented explicitly per
  newtype (`Clipboard`, `Http`, `CursorCap`).
- `Plugin::requires(&mut CapabilityDecls)` — declare hard deps; the builder
  validates them BEFORE any plugin's `register` runs, so missing capabilities
  fail fast at `build()` with a clear error.
- `Capabilities::of::<C>()` / `require::<C>()` — deferred lookup at JS call
  time (bridge fns) or event dispatch time (subsystems via
  `SubsystemFlushContext.capabilities`).
- Convention: capability newtypes use base names (`Clipboard`, `Http`);
  backend traits use `*Backend` suffix (`ClipboardBackend`, `HttpBackend`,
  `CursorBackend`). `CursorCap` is the lone exception because
  `core::platform::Cursor` already names the cursor-kind enum.


### Element types

`Column`, `Row`, `Expanded`, `Stack`, `Positioned`, `SizedBox`, `Container`, `PointerInteract`, `Focusable`, `Text`, `Input`, `Paragraph`, `Image`, `Svg` (all in `tur-engine::builtin_plugins::*`) · `Opacity`, `Transform` (tur-animation)

Flutter-like layout model: flex-based Column/Row with Expanded children, Stack with Positioned children.

### Animation model (Flutter-aligned)

Animation lives entirely in the standalone `tur-animation` crate (registered via `TurAnimationPlugin`). The engine core exposes only the `Subsystem` flush hook + `Clock` accessor — no animation code is in `tur-engine`.

- **`Subsystem` trait** (`tur-engine::core::subsystem`) — one trait, three methods, all defaulting to no-op:
  - `fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) -> SubsystemOutcome` — called once per `flush()` call (= once per frame), in registration order. Used for time-driven state advance. `AnimationSubsystem` owns `AnimationManager` + the engine `Clock` and ticks the manager each flush.
  - `fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent)` — called per drained platform event, every fixed-point iteration, in registration order. Used by input subsystems (keyboard, IME, gesture, pointer, scroll, resize, clipboard platform-bridge).
  - `fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent)` — called per drained engine-internal event, every fixed-point iteration, in registration order. Used by scroll-chaining / scroll-to / clipboard-write / clipboard-paste / caret-visibility subsystems.

  `SubsystemFlushContext` exposes the boa `Context`, the element tree / focus manager / mutation queue (as shared `Rc<RefCell<>>` so subsystems that already hold their own Rc clone — like `AnimationSubsystem` capturing the mutation queue for `onTick` callbacks — don't panic on a double-borrow), both event queues, the renderer, the canvas size, the async executor, and the capability registry.
- **`Curve`** (`tur-animation::curve`) — a time-remap `f64 → f64` (Flutter `Curve`): `Linear`/`EaseIn`/`EaseOut`/`EaseInOut`. Parsed from JS strings like `"easeInOut"`.
- **`Tween<T>`** (`tur-animation::tween`) — a value range `{begin, end}` with `lerp(t) → T` (Flutter `Tween<T>`). `NumTween` for `f64`, `ColorTween` for component-wise `Color` interpolation via `Color::lerp`. Exposed in JS as `Tween({begin, end})` / `ColorTween({begin, end})` with mutable `begin`/`end` and `lerp`/`transform` methods.
- **Effect elements**: `Opacity` (alpha-mask a child) and `Transform` (rotate/scale/translate). Registered by `tur-animation` under `tur:animation`.
- **Explicit animation**: `createAnimationController({duration, curve, repeat, onTick, onEnd})` drives a source atom via `onTick`; pair with `Tween.lerp(t)` in a `derive()` for explicit, controller-driven interpolation (continuous loops, transport controls). See the `complex-animation` case.
- **Implicit animation** (JS, in `tur-animation`'s `js/index.js`): `AnimatedContainer` / `AnimatedOpacity` / `AnimatedPositioned` wrap their plain siblings (`Container` / `Opacity` / `Positioned`). Each animatable prop is a `Tween` channel displayed as `tween.lerp(progress)`; one shared `progress` source is driven by a single `AnimationController`'s `onTick`. `ReadableSubscribe` watches the reactive targets — on change, `onUpdate$` rebases each channel's `begin` to its currently-displayed value, sets `end` to the new target, and restarts the controller (Flutter's `ImplicitlyAnimatedWidget` retarget). Static props pass through. See the `implicit-animations` case.

`tur-animation` registers ONE combined consumer-facing module `tur:animation` (JS source loaded via `include_str!` + `register_js_module`) that re-exports native fns (`Opacity`, `Transform`, `createAnimationController`) from the hidden `tur:animation/native` module and defines the JS widgets on top.

### Text model

Text logic lives in `tur-engine::builtin_plugins::text` (inlined from the former `libs/tur-text` crate). It is installed into `tur:std` by `TurStdPlugin` via `install_text(ctx: &mut PluginContext) -> Result<Vec<FnEntry>, TurError>`. The returned `FnEntry`s are merged into `std_fns` before `register_module("tur:std", ...)`, so `Text` / `Input` / `createTextEditingController` / `createUndoController` ship as part of the std module from JS's perspective.

- **Engine contract types** (kept in `tur-engine::core::text::text_layout` + `core::fonts`): `TextLayoutData`, `LineInfo`, `LineGlyphStop`, `TextRunData`, `TextGlyph`, `FontManager`, `FontLoader`. The engine's `Canvas::fill_text_layout(&TextLayoutData)` does the actual drawing; the text plugin only produces these structs.
- **`extract_layout_data(props) -> TextLayoutData`** (`builtin_plugins/text/text_layout.rs`): bridge helper that turns JS-side text props into the engine's `TextLayoutData` used by layout + paint.
- **Elements** (`builtin_plugins/text/elements`): `TextElement` (static text), `EditableTextElement` (cursor + selection + IME + paste), `ParagraphElement`.
- **Controllers** (`builtin_plugins/text/controller`): `TextEditingController` (registered class — `register_class`), `UndoController`, plus `SpanData` + event types.
- **Post-event caret visibility** (`builtin_plugins/text/handlers`): `CaretVisibilitySubsystem` runs after keyboard/IME/paste subsystems (in registration order) and scrolls the focused editable's `ScrollView` to keep the caret in view. The engine's `builtin_plugins/input/{subsystem.rs,ime.rs}` no longer call caret-scroll directly.
- **Paste dispatch** (embedder → tur-clipboard → text plugin): the embedder wraps the platform paste as a `ClipboardPlatformPasteEvent` (carried inside `PlatformEvent::Custom`) and pushes it onto the platform queue. tur-clipboard's `ClipboardPlatformSubsystem` (in `builtin_plugins::clipboard::handlers`, registered by `TurClipboardPlugin`) consumes it and re-emits a `ClipboardPasteEvent` (carried inside `AppEvent::Custom`) on the engine-internal bus. The text plugin's `ClipboardPasteSubsystem` (`builtin_plugins/text/handlers`) consumes the AppEvent, looks up the focused `EditableTextElement`, and inserts the text (replacing any selection, or at the caret). No per-element trait is needed: paste is a single-consumer, stateless op. The engine stays free of any text-element *and* clipboard knowledge — domain-specific events travel through the `Custom` escape hatches on `PlatformEvent` / `AppEvent` (typed by the `CustomPlatformEvent` / `CustomAppEvent` traits). The event payload types themselves live in `builtin_plugins::clipboard::event` (clipboard-plugin-owned; cross-plugin via `pub(in crate::builtin_plugins)`).

JS surface is unchanged — `tur:std` still exports Text/Input/etc. No `.d.ts` split, no new JS package.

### Image model

Image logic lives in `tur-engine::builtin_plugins::image` (inlined from the former `libs/tur-image` crate). It is installed into `tur:std` by `TurStdPlugin` via `install_image(ctx: &mut PluginContext) -> Result<Vec<FnEntry>, TurError>`. The returned `FnEntry`s are merged into `std_fns` before `register_module("tur:std", ...)`, so `Image` / `createImageResource` / `createSvgResource` ship as part of the std module from JS's perspective.

- **Engine contract types** (kept in `tur-engine::core::image_resource`): `ImageResourceId`, `ImageResourceMap`, `ImageResource`. The struct's `peniko_image` / `natural_size` fields are `pub` (matching `TextLayoutData`). `Canvas::draw_image(ImageResourceId, natural_size, transform)` does the actual drawing; the image plugin only produces these structs.
- **Engine retains `from_rgba(raw, w, h) -> ImageResource`** as the constructor for raw RGBA pixels — pure data, no format-decoder deps.
- **Decoders** (`builtin_plugins/image/decode`): `decode_image_bytes(&[u8])` (PNG/JPEG via the `image` crate) and `decode_svg(&str)` (rasterised via `usvg` + `resvg`). The `image` / `resvg` / `usvg` deps live in `tur-engine`'s Cargo.toml.
- **Element** (`builtin_plugins/image/element`): `ImageElement` + `ImageView` + layout (`ElementLayout`) + paint (`ElementRender`) including `BoxFit` math. The engine's `PaintContext::get_image_resource(ImageResourceId)` and `LayoutContext::get_image_natural_size(ImageResourceId)` are the lookup hooks; `TurJsContext::image_resource_map()` is the public accessor the JS bridge uses to call `insert_image`.
- **Resource storage is image-only**: `ImageResourceMap` is a flat `HashMap<ImageResourceId, ImageResource>` — there is no `Resource` enum wrapper because images are the only resource kind.

JS surface is unchanged — `tur:std` still exports Image/createImageResource/createSvgResource. No `.d.ts` split, no new JS package.

### Domain traits

Each element implements these focused traits:

- `ElementOnUpdate` — JS property mutation (`set_prop`)
- `ElementLayout` — layout (`perform_layout`: measure children, compute own size, assign child offsets in one pass)
- `ElementRender` — painting and hit testing (`paint`, `hit_test`, `type_name`)
- `ElementSubscribe` — declares which reactive atoms the node depends on (`subscribe`), so a reactive flush can mark it dirty for re-layout. Runs as an explicit phase after `perform_layout` for dirty nodes.

Elements are type-erased via `AnyElement` (private `Erased` trait with blanket impl for all domain traits). Paste is **not** an element trait — it flows through a `ClipboardPasteEvent` (inside `AppEvent::Custom`) + tur-text's `ClipboardPasteSubsystem` (see [Text model](#text-model)).

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
      core/                  # Engine infrastructure — NO dependency on
                             #   builtin_plugins/* (strict boundary)
        app/                 # TurAppInternal + FrameOutcome + AppEvent/
                             #   AppEventQueue + render() mount +
                             #   RootView/RootElement generic-root wrapper
        async_/              # AsyncExecutor (tur_async wrapper) + task
                             #   primitives (sleep/launch, the bridge fns
                             #   for tur:std) + executor
                             #   (TurJobExecutor — boa JobExecutor impl)
        capability.rs        # Capability trait, Capabilities view,
                             #   CapabilityDecls
        dev/                 # Dev tooling: turDevTool bridge
        edgy/                # Reactive substrate: reactive/ (Store/Source/
                             #   Derived/AnyReadable) + mutation/
                             #   (MutationHandle/PendingMutationInvocationQueue)
                             #   + source/derive/mutate/get/set/view bridge +
                             #   ReadableSubscribe (the engine's own
                             #   tur:core)
        element.rs           # ElementKind / ElementNodeId / NodeId /
                             #   FragmentNodeId
        elements/            # AnyElement, ElementObject, ElementTree
        focus/               # FocusManager + Focusable trait +
                             #   BlurEvent/FocusEvent/FocusChange
                             #   (engine contract — the Focusable *widget*
                             #   lives in builtin_plugins/focus)
        fonts.rs             # FontManager + FontLoader (used by
                             #   Canvas::fill_text_layout)
        hit_test/            # hit-test primitives
        image_resource.rs    # ImageResourceId / ImageResourceMap /
                             #   ImageResource (paint/layout contract)
        js_runtime/          # boa runtime plumbing: TurJsContext, JsProps,
                             #   FnEntry/ConstEntry, module_loader
                             #   (build_native_module/bound_native),
                             #   opaque (BoaOpaque),
                             #   js_value (FromJs/IntoJs).
                             #   Shared by every bridge fn engine-wide.
        layout/              # ElementLayout, ElementSubscribe, LayoutContext,
                             #   primitives (Constraints/Offset/Size/
                             #   EdgeInsets/Axis/MainAxisAlignment/…),
                             #   SubscribeCx
        platform/            # Cursor/CursorBackend/CursorCap +
                             #   PlatformEvent/PointerInput/ImeEvent +
                             #   PlatformEventQueue (raw input from embedder) +
                             #   key_event.rs (KeyEvent/Modifiers/
                             #   KeyEventType/KeydownEvent/KeyupEvent —
                             #   engine contract types)
        plugin.rs            # Plugin trait (register + requires) + PluginContext
        render/              # PaintContext, Renderer, ElementRender trait,
                             #   Canvas + brush/ (Color/Brush/GradientStop/
                             #   RGB types + JS bindings)
        screen/              # Screen struct (logical_size + viewportSize$
                             #   source atom) + ResizeSubsystem (handles
                             #   PlatformEvent::Resize)
        shell/               # Shell (engine-internal scheduler/clock holder)
        subsystem.rs         # Subsystem trait (flush +
                             #   handle_platform_event + handle_app_event) +
                             #   SubsystemFlushContext + SubsystemOutcome
        text/                # TextLayoutData + LineInfo + TextRunData
                             #   (paint/layout contract types only — the
                             #   text plugin produces them)
        view/                # View/ViewCx/SharedViewCx + Val<T> + Lifecycle
      builtin_plugins/       # Feature bundles — each exposes ONE
                             #   `pub fn install_xxx(ctx) -> Result<Vec<FnEntry>, TurError>`
                             #   so `core/` cannot import from this tree
        std.rs               # TurStdPlugin — the orchestrator that calls
                             #   every install_xxx and merges FnEntry into
                             #   tur:std
        clipboard/           # Clipboard capability + ClipboardBackend trait +
                             #   TurClipboardPlugin + tur:clipboard +
                             #   event payloads + engine-internal subsystems
                             #   (inlined from former tur-clipboard-capability
                             #   crate). Public surface (Clipboard /
                             #   ClipboardBackend / TurClipboardPlugin /
                             #   platform_paste) re-exported at tur_engine
                             #   crate root.
        console.rs           # Global `console` object (log/warn/error/info/
                             #   debug) — install_console registers globals
        control_flow/        # Condition, Switch, Each, Fragment
        focus/               # Focusable widget (manager + trait are in core)
        gesture/             # MouseRegion + PointerInteract +
                             #   GestureSubsystem + PointerSubsystem
        image/               # ImageElement + ImageView + PNG/JPEG/SVG
                             #   decoders (inlined from former tur-image crate)
        input/               # KeyboardSubsystem + ImeSubsystem (event types
                             #   are in core::platform::key_event)
        layout/              # Column/Row (flex), Expanded, Stack, Positioned,
                             #   Container/SizedBox + JS layout enums
                             #   (Axis/MainAxisAlignment/…)
        lazy_container/      # LazyList + LazyListController (inlined from
                             #   former tur-lazy-container crate)
        lifecycle/           # lifecycleView (mount/unmount callbacks)
        scroll/              # ScrollView, Scrollbar, ScrollController,
                             #   ScrollSubsystem (inlined from former
                             #   tur-scroll crate)
        text/                # TextElement, EditableTextElement,
                             #   ParagraphElement, controllers,
                             #   ClipboardPasteSubsystem,
                             #   CaretVisibilitySubsystem (inlined from
                             #   former tur-text crate)
      renderer/
        vello/               # VelloRenderer (GPU painting)
        noop/                # NoopRenderer (logging)
  tur-animation/             # Animation subsystem (manager/controller/event +
                             #   Opacity/Transform effects + JS widgets +
                             #   Curve/NumTween/ColorTween) — registered via
                             #   TurAnimationPlugin, exposes
                             #   `tur:animation` (combined native+JS
                             #   module) + internal `tur:animation/native`
                             #   (ctx-bound fns only)
  tur-clipboard-wasm/        # WasmClipboard (navigator.clipboard) backend —
                             #   re-exports Clipboard/ClipboardBackend/
                             #   TurClipboardPlugin from tur_engine
  tur-clipboard-native/      # NativeClipboard (arboard) backend — same
                             #   re-exports
  tur-net-capability/        # HttpBackend trait + Http cap + tur:net
  tur-net-wasm/              # WasmHttp (reqwest-wasm) backend
  tur-net-native/            # NativeHttp (reqwest) backend
  tur-wasm/                  # wasm binary (boa_engine + vello-hybrid +
                             #   tur-engine). Composes Clipboard + Http +
                             #   CursorCap backend capabilities with
                             #   `tur_engine::TurStdPlugin` (re-exported
                             #   from `builtin_plugins::std`) →
                             #   TurAnimationPlugin → TurClipboardPlugin →
                             #   TurNetPlugin.
  tur-integration-tests/     # integration test harness + cases
  tur-demo-plugin/           # playground-only plugin (swc compiler + file IO)
  tur-native/                # native (non-wasm) embedder entry point
js/
  packages/
    tur-animation/            # Ambient TS types for `tur:animation`
                             #   (runtime provided by tur-animation crate)
    tur-core/                # Ambient TS types for `tur:core`
                             #   (engine-owned reactive primitives)
    tur-demo/                # Playground: thin browser wrapper (loads wasm
                             #   + impl bundle)
    tur-demo-impl/           # Playground UI built with tur:animation
                             #   + tur:std (Sidebar/Editor/Viewer)
    tur-test-cases/          # Test cases (cases/, ~60 cases)
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
cd js/packages/tur-demo-impl && pnpm build
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


