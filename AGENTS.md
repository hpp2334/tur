# tur

A JavaScript rendering engine built with winit, vello-hybrid, and boa_engine. JS calls into the engine via the `tur:std` / `tur:animation` / `tur:clipboard` / `tur:net` / `tur:filepicker` modules registered by engine plugins.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  demo/website (web host app — @tur-ng/website)            │
│  Thin browser host: loads the tur WASM + the           │
│  playground-view bundle. Co-located with its own        │
│  wasm cdylib (demo/website/native → tur-website).       │
├─────────────────────────────────────────────────────┤
│  demo/playground-view (@tur-ng/playground-view)           │
│  The playground view: UI built with tur:animation +     │
│  tur:std (Sidebar/Editor/Viewer) + inlined case         │
│  sources. A reusable view the website renders.          │
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
│  │   ├── js_runtime/ (boa plumbing: TurInstanceContext,      │
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
│  │   ├── subsystem.rs (Subsystem trait + flush_pre/post_layout hooks)   │
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
│     ├── tur-net-native         (NativeHttp via reqwest on a│
│     │   user-provided tokio runtime — the only crate that │
│     │   touches tokio; engine core is tokio-free)        │
│     ├── tur-filepicker-capability (FilePicker +         │
│     │   FilePickerBackend trait + tur:filepicker bridge │
│     │   — opt-in, requires a backend)                    │
│     ├── tur-filepicker-wasm   (WasmFilePicker via web-sys)│
│     └── tur-filepicker-native (NativeFilePicker via rfd)│
│  Backend crates for the inlined Clipboard cap:         │
│     ├── tur-clipboard-wasm  (WasmClipboard; re-exports │
│     │   Clipboard/ClipboardBackend/TurClipboardPlugin) │
│     └── tur-clipboard-native (NativeClipboard via      │
│         arboard; same re-exports + AsyncPluginContext; │
│         new(&cx) self-hops each read/write to main)    │
│  Embedders register backends via .capability(|cx|...): │
│    the closure receives &AsyncPluginContext -- backends │
│    needing main take cx (NativeClipboard self-hops);   │
│    the rest ignore it (WasmClipboard/Http/FilePicker). │
│    Engine creates the channel internally in build();   │
│    no main_handle/main_drain builder wiring needed.    │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-wasm (pure rlib — the reusable wasm embedder) │
│  No #[wasm_bindgen] surface, no playground code.       │
│  Owns all DOM wiring + the WebGL2 renderer + WasmClock  │
│  / WasmFontLoader / WasmCursor + the standard           │
│  capability backends (WasmClipboard / WasmHttp /        │
│  WasmFilePicker).                                       │
│  Exposes WasmAppHandle::create(WasmAppConfig) — a        │
│  builder taking a `configure` callback (extra plugins)   │
│  + optional after-frame hook. The host cdylib wraps it.  │
│  Composes the default plugin chain:                       │
│  TurStdPlugin → TurAnimationPlugin → TurClipboardPlugin │
│  → TurNetPlugin → TurFilePickerPlugin.                   │
├─────────────────────────────────────────────────────┤
│  demo/website/native (tur-website cdylib — the host .so) │
│  The website's own wasm entry: wraps WasmAppHandle,      │
│  adds TurDemoPlugin (swc compiler only). File IO now     │
│  lives in tur:filepicker (registered by tur-wasm).       │
│  Exports #[wasm_bindgen] TurWebsiteApp (create /         │
│  create_in / loadAndRunModule / dev_tool) → tur_website.js.│
│  Mirrors tur-android (rlib) + demo/compose/native (cdylib).│
└─────────────────────────────────────────────────────┘
```

### Capability registry

Embedders register swappable backends (clipboard, http, filepicker) on the runtime builder (shared across all instances spawned from the runtime). Registration is **closure-based**: `.capability(|cx: &AsyncPluginContext| Result<C, TurError>)`. The closure runs once in `build()` (after the engine creates its internal main-thread channel) and receives an `AsyncPluginContext` — the engine's main-thread hop. Backends that need to run OS-API calls on main (e.g. `NativeClipboard` on macOS, where `arboard`/`NSPasteboard` require main-thread access) store a clone and self-hop via `cx.run_on_main(...)`; the rest (wasm, HTTP via tokio, filepicker via `rfd`) ignore the argument. The cursor is per-instance (set via `TurApp::set_cursor_backend` after `app_builder().build(...)`, since it targets a specific surface):

```rust
let runtime = TurRuntime::builder()
    .font_loader(Rc::new(WasmFontLoader::new()))
    .clock(Rc::new(WasmClock))
    .capability(|_| Ok(Clipboard::new(WasmClipboard)))   // tur-clipboard-wasm
    .capability(|_| Ok(Http::new(WasmHttp)))             // tur-net-wasm
    .capability(|_| Ok(FilePicker::new(WasmFilePicker))) // tur-filepicker-wasm
    // A backend that needs main-thread access takes the context:
    //   .capability(|cx| Ok(Clipboard::new(NativeClipboard::new(cx)?)))
    .plugin(TurStdPlugin)
    .plugin(TurAnimationPlugin)                  // tur-animation (after TurStdPlugin)
    .plugin(TurClipboardPlugin)                  // requires: Clipboard
    .plugin(TurNetPlugin)                        // Http optional (skips tur:net if absent)
    .plugin(TurFilePickerPlugin)                 // requires: FilePicker
    .build()?;                                    // Rc<TurRuntime>

// Spawn isolated instances (each its own JS realm + renderer):
let app = runtime
    .app_builder()
    // Optional: define build-time per-instance data readable/updateable by
    // plugins/bridge fns via `TurInstanceContext::data::<T>()` /
    // `with_data::<T, _>(f)` / `update::<T>(v)`. Each type may be defined
    // exactly once (duplicate `define` panics). The closure RUNS ON THE
    // WORKER (right after the instance is constructed, before any plugin
    // `register`), so values built fresh in the body never cross the
    // main↔worker boundary.
    //   .instance_data(|cx| {
    //       cx.define::<PluginId>(PluginId("com.example.foo".into()));
    //   })
    .renderer(Box::new(renderer), (800.0, 600.0), 2.0)  // group all three
    .build()?;
app.set_cursor_backend(Rc::new(RefCell::new(WasmCursor { canvas })));  // per-instance

// Or a headless instance (no rendering):
let headless = runtime.app_builder().build_headless((0.0, 0.0))?;
```

- `Capability: Any + Clone + 'static` — marker trait, implemented explicitly per
  newtype (`Clipboard`, `Http`, `FilePicker`, `CursorCap`).
- `AsyncPluginContext` (`core::plugin`, re-exported at the crate root) — the
  engine's `Send + Sync + Clone` main-thread hop. The engine creates the
  channel internally in `build()` and spawns the drain on the main thread, so
  **no embedder wiring is required** (no `main_handle`/`main_drain` builder
  methods). OS-API backends receive a clone at construction (via the
  capability closure) and self-hop; plugin/bridge code reaches the same
  channel via `PluginContext::to_async()`. The raw `MainTask`/`MainDrain`/
  `main_channel()` live `pub(crate)` in `core::scheduler` (the plugin layer
  wraps the sender — dependency direction: plugin → scheduler, never reverse).
- `Plugin::requires(&mut CapabilityDecls)` — declare hard deps; the builder
  validates them BEFORE any plugin's `register` runs, so missing capabilities
  fail fast at `build()` with a clear error. (`TurNetPlugin` is the exception —
  it feature-detects `Http` at `register` and skips `tur:net` if absent, rather
  than declaring `requires`; `TurClipboardPlugin` / `TurFilePickerPlugin` use
  the strict `requires` form.)
- `Capabilities::of::<C>()` / `require::<C>()` — deferred lookup at JS call
  time (bridge fns) or event dispatch time (subsystems via
  `SubsystemFlushContext.capabilities`).
- **Per-instance data** (build-time `InstanceDataCx::define::<T>(value)` →
  runtime `TurInstanceContext::update::<T>(value)` /
  `data::<T>()` / `with_data::<T, _>(f)`) — typed worker-side metadata with
  a strict **build-time define / runtime update+read** split:
  - **Build time** (`TurAppBuilder::instance_data(|cx| cx.define::<T>(v))`):
    the ONLY way to introduce a new `TypeId` into the map. The closure runs
    on the worker (right after `TurInstanceContext` is constructed, before
    any plugin `register`), so values built fresh in the body never cross
    the main↔worker boundary; only captured values need `Send`. Each type
    may be defined exactly once per instance — duplicate `define` panics
    (fail-fast). Plugins see all defined slots as already-present at
    `register` time.
  - **Runtime** (`TurInstanceContext::update::<T>(v)`): replace an existing
    value; panics if the `TypeId` was NOT defined at build time (catches
    missing `define` immediately).
  - **Runtime read** (`data::<T>()` returns `Option<T>` (requires `T: Clone`);
    `with_data::<T, _>(f)` is the no-`Clone`-bound ref-callback path).
  Carries secure, JS-unforgeable identity (e.g. a host `PluginId` so a
  `storage.get(key)` bridge can resolve the calling plugin without trusting
  JS args). Mirrors the `Capabilities` shape: `Rc<RefCell<HashMap<TypeId,
  Box<dyn Any>>>>` inside `TurInstanceContext`, shared across every cheap clone.
  Lives entirely in the worker.
- Convention: capability newtypes use base names (`Clipboard`, `Http`,
  `FilePicker`); backend traits use `*Backend` suffix (`ClipboardBackend`,
  `HttpBackend`, `FilePickerBackend`, `CursorBackend`). `CursorCap` is the lone
  exception because `core::platform::Cursor` already names the cursor-kind enum.


### Reactive substrate (plugin-facing atom minting)

The reactive substrate (`core::edgy`) is engine-owned per-instance infrastructure
(like `mutation_queue` / `clock` / `event_bus`), NOT a swappable cross-cutting
backend. Plugins mint reactive atoms from Rust via the narrow
`ReactiveBridgeStore` face returned by `PluginContext::reactive()` (or
`TurInstanceContext::reactive()` from inside a bridge fn):

```rust
fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
    let bridge = ctx.reactive();

    // Mint a source. Initial value is a JsValue (the store is type-erased to
    // JsValue at runtime; Source<T>'s T is a type-level marker only).
    let clock: Source<JsValue> = bridge.source(JsValue::new(0.0));

    // Expose to JS — handles cross the boundary via IntoJs (opaque JsObject).
    // JS reads via the unchanged `get(mySource)` from `tur:core` / `tur:std`.
    let js_handle = clock.into_js(ctx.boa_mut());
    ctx.register_global("clock$", js_handle);

    // A subsystem can write to the source from Rust each frame — mirrors the
    // engine-internal `viewportSize$` pattern (Screen::sync_source on resize).
    ctx.register_subsystem(Box::new(ClockSubsystem { source: clock, bridge }));
    Ok(())
}
```

The JS side is unchanged — atoms minted by Rust are indistinguishable from
atoms minted by JS. JS reads/writes via `get(atom)` / `set(source, v)` /
`set(mutation, ...args)` / `ReadableSubscribe(...)`.

**Rust-native closures** (`build_derive` / `build_mutate`) skip the `{get, set}`
JsObject round-trip that JS `derive(fn)` / `mutate(fn)` closures pay. The
closure receives a typed capability face directly:

- `bridge.build_derive(F)` where `F: Fn(&ReactiveReadStore, &mut Context) -> JsResult<JsValue>`
  — read-only face; reads inside the closure flow through `ReactiveCore::read`,
  so the auto-dependency tracker (`tracker_stack`) records them as it would for
  a JS closure. No manual dep declaration.
- `bridge.build_mutate(F)` where `F: Fn(&ReactiveBridgeStore, &[JsValue], &mut Context) -> JsResult<JsValue>`
  — read+write face + the user-supplied args verbatim (no JsObject prepended).

```rust
let flag: Source<JsValue> = bridge.source(JsValue::new(false));
let flag_for_closure = flag;
let bridge_for_closure = bridge.clone();
let toggle = bridge.build_mutate(move |b, _args, boa| {
    let current = b.read(Readable::from(flag_for_closure), boa).as_boolean().unwrap_or(false);
    bridge_for_closure.set_source(flag_for_closure, JsValue::new(!current));
    Ok(JsValue::undefined())
});
// JS invokes via `set(globalThis.toggle)` — invoke_mutation detects the
// MutateRust variant and hands the closure the bridge face + user args.
```

Implementation notes:
- The `Js` and Rust closure variants share `ReactiveCore` storage as a
  `Closure` enum (`Js(JsFunction)` / `DeriveRust(Rc<dyn Fn>)` /
  `MutateRust(Rc<dyn Fn>)`); the kind is encoded in the variant so cross-kind
  dispatch is unreachable via the public API (handle types `Derived<T>` vs
  `Mutation` make it impossible to mismatch; defensive panics guard engine
  bugs).
- `invoke_mutation` builds the per-store `{get, set}` JsObject **internally**
  only for `Js`-variant closures. Callers pass user args verbatim (no
  prepend) — see `core::edgy/reactive/store.rs::ReactiveCore::invoke_mutation`.
- Rust closures are `Rc<dyn Fn>` (not `Box<dyn Fn>`) so the existing
  clone-out-before-call discipline (matching `JsFunction::clone()`) is
  preserved — this is what makes nested `ensure_computed` (a derive closure
  reading another derived) safe under RefCell.
- Closures are `!Send` (`Rc`-captured); they live entirely on the worker
  thread. Matches the `Rc<RefCell<...>>` discipline throughout the substrate
  and the `!Send` `JsFunction` path.

### Multi-instance model (TurRuntime + TurApp)

The engine has a **one runtime, many instances** architecture:

- **`TurRuntime`** (`tur-engine::core::runtime`) — the shared, created-once
  substrate. Owns the `FontContext` (system-font discovery + preset fonts, built
  once — each instance clones it cheaply; `FontContext`/`fontique::Collection`/
  `System` are all `Arc`-backed), the `Clock` (one shared time source), the
  `Capabilities` registry (shared Clipboard/Http/FilePicker backends), and the
  registered `Plugin`s. Built via `TurRuntime::builder()...build()`.
- **`TurApp`** — an isolated instance spawned from a runtime via
  `runtime.app_builder().build(renderer, viewport, dpr)` (rendering, attached to
  a surface) or `runtime.app_builder().build_headless(viewport)` (no rendering —
  JS + capabilities + events only, backed by `NoopRenderer`). Each instance
  gets its own boa `Context` (JS realm), element tree, reactive store, focus
  manager, event queues, subsystems, screen, and scheduler (per-instance vsync
  drivers — e.g. Android instances install their own JNI `FrameLoop`-bound
  scheduler via `TurApp::set_main_scheduler`). Plugins are re-registered
  into each instance's fresh realm (the same plugin objects — `register` takes
  `&self`, so no factory needed).

The `Plugin` trait has two phases: `compile` (called once on the runtime —
pre-validate/cache) and `register` (called per instance — into the fresh boa
`Context`). boa `Module`s are realm-bound, so the actual JS parse happens per
instance in `register`; `compile` is the seam for future caching + fail-fast
validation. The renderer is **not** on the runtime builder — it's the
mandatory argument to the builder's `build(...)` terminal (one renderer per
surface). The cursor backend is per-instance (set via
`TurApp::set_cursor_backend` after spawn, since it targets a specific surface).

Embedder splits mirror this: `AndroidRuntime`/`AndroidInstance` (tur-android),
`WasmRuntime`/`WasmApp` (tur-wasm), and `TurRuntime`/`TurInstance` +
`rememberTurRuntime` (Compose). The integration tests under
`tests/element/multi_instance.rs` pin the isolation guarantees.


### Element types

`Column`, `Row`, `Expanded`, `Stack`, `Positioned`, `SizedBox`, `Container`, `PointerInteract`, `Focusable`, `Text`, `Input`, `Paragraph`, `Image`, `Svg` (all in `tur-engine::builtin_plugins::*`) · `Opacity`, `Transform` (tur-animation)

Flutter-like layout model: flex-based Column/Row with Expanded children, Stack with Positioned children.

### Animation model (Flutter-aligned)

Animation lives entirely in the standalone `tur-animation` crate (registered via `TurAnimationPlugin`). The engine core exposes only the `Subsystem` flush hooks (`flush_pre_layout` / `flush_post_layout`) + `Clock` accessor — no animation code is in `tur-engine`.

- **`Subsystem` trait** (`tur-engine::core::subsystem`) — one trait, four methods, all defaulting to no-op:
  - `fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext<'_>)` — returns nothing; called **every fixed-point iteration** of `flush()` (possibly several times per frame), in registration order, **before** the layout step. Used for time-driven state advance. `AnimationSubsystem` owns `AnimationManager` + the engine `Clock` and advances the manager at most once per frame, self-gating via `cx.frame_id()` (a per-`flush()` epoch stable across iterations, differing across frames). Subsystems push intent back into the engine via `cx.mark_dirty()` (re-layout + paint this iteration), `cx.request_paint()` (paint this frame), and `cx.request_next_frame()` (schedule the next vsync — accumulates across all iterations and feeds the post-loop schedule decision). Emitting `request_next_frame()` every iteration a controller is active is what keeps an animation started from a callback (event/lifecycle handler) advancing without waiting for the next platform input.
  - `fn flush_post_layout(&mut self, cx: &mut SubsystemFlushContext<'_>)` — same cadence + registration order, but **after** the layout step, so it reads the freshly-laid-out tree (`computed_layout`, `absolute_affine_of`). Used for layout-derived recomputation — e.g. `CompositedTransformSubsystem` maps each target's world position onto its follower with final geometry + the follower's just-resolved anchor cache. Without this phase a follower read zero/stale sizes on the first frame and only self-corrected on the next input event (tap/click) — see `follower_correct_on_first_frame_non_topleft_anchor`.
  - `fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent)` — called per drained platform event, every fixed-point iteration, in registration order. Used by input subsystems (keyboard, IME, gesture, pointer, scroll, resize, clipboard platform-bridge).
  - `fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent)` — called per drained engine-internal event, every fixed-point iteration, in registration order. Used by scroll-chaining / scroll-to / clipboard-write / clipboard-paste / caret-visibility subsystems.

  `SubsystemFlushContext` exposes the boa `Context`, the element tree / focus manager / mutation queue (as shared `Rc<RefCell<>>` so subsystems that already hold their own Rc clone — like `AnimationSubsystem` capturing the mutation queue for `onTick` callbacks — don't panic on a double-borrow), both event queues, the renderer, the canvas size, the async executor, the capability registry, plus the engine-signalling channels `mark_dirty` / `request_paint` / `request_next_frame` and the `frame_id()` self-gate. These channels are bundled in `FlushSignals` (built once per `flush()` and shared with every context constructed that call).
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
- **Element** (`builtin_plugins/image/element`): `ImageElement` + `ImageView` + layout (`ElementLayout`) + paint (`ElementRender`) including `BoxFit` math. The engine's `PaintContext::get_image_resource(ImageResourceId)` and `LayoutContext::get_image_natural_size(ImageResourceId)` are the lookup hooks; `TurInstanceContext::image_resource_map()` is the public accessor the JS bridge uses to call `insert_image`.
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
        async_/              # CompletionQueue/CompletionHandle (pending
                             #   completion invocations drained each flush)
                             #   + executor (TurJobExecutor — boa
                             #   JobExecutor impl)
          scheduler.rs         # MainSchedulerDriver + WorkerSchedulerDriver
                               #   traits + MainScheduler/WorkerScheduler view
                               #   structs + WorkerFactory +
                               #   Sleep/VsyncEvents/WorkerHandle +
                               #   SpawnError/TaskHandle/track_spawn (generic
                               #   task tracking) + the raw main-thread hop
                               #   mechanics (pub(crate) MainTask/MainDrain/
                               #   main_channel — the plugin-layer
                               #   AsyncPluginContext wraps the sender)
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
        js_runtime/          # boa runtime plumbing: TurInstanceContext, JsProps,
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
                             #   + CompileContext + AsyncPluginContext
                             #   (Send+Sync+Clone main-thread hop — wraps the
                             #   scheduler's pub(crate) channel sender; the
                             #   engine creates the channel internally in
                             #   build() so no embedder wiring is needed) +
                             #   MainRunFuture + PluginContext::to_async()
        render/              # PaintContext, Renderer, ElementRender trait,
                             #   Canvas + brush/ (Color/Brush/GradientStop/
                             #   RGB types + JS bindings)
        screen/              # Screen struct (logical_size + viewportSize$
                             #   source atom) + ResizeSubsystem (handles
                             #   PlatformEvent::Resize)
        shell/               # Shell (engine-internal scheduler/clock holder)
        subsystem.rs         # Subsystem trait (flush_pre_layout +
                             #   flush_post_layout + handle_platform_event +
                             #   handle_app_event) +
                             #   SubsystemFlushContext + FlushSignals
                             #   (subsystems signal via cx.mark_dirty /
                             #    request_paint / request_next_frame; flush
                             #    returns () — no SubsystemOutcome)
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
                             #   re-exports + AsyncPluginContext.
                             #   NativeClipboard::new(&AsyncPluginContext)
                             #   stores it and self-hops each read/write to
                             #   main (macOS NSPasteboard needs main-thread)
  tur-net-capability/        # HttpBackend trait + Http cap + tur:net
  tur-net-wasm/              # WasmHttp (reqwest-wasm) backend
   tur-net-native/            # NativeHttp (reqwest) backend — runs each request
                              #   on a user-provided tokio runtime (Handle) and
                              #   bridges results back via oneshot/mpsc; the only
                              #   crate in the workspace that touches tokio
   tur-clipboard-android/     # AndroidClipboard (ClipboardManager via JNI) —
                             #   registers the process JavaVM for per-call attach
   tur-android/               # Embedder glue (rlib, NOT a cdylib): wgpu/Vulkan
                             #   over an Android Surface + the JNI event/loop
                             #   bridge. Provides `ops::create_with_plugins`
                             #   (engine build with an injectable plugin set +
                             #   Android-default capabilities) + the standard-op
                             #   `pub fn`s + the `standard_jni_exports!()` macro
                             #   that generates `Java_org_tur_TurNative_*`
                             #   trampolines inside an app's own cdylib. No
                             #   plugins hardcoded (was: cdylib with demo plugin).
    tur-wasm/                  # Pure reusable rlib (NOT a cdylib): the wasm
                              #   embedder lib. Owns all DOM wiring + the
                              #   WebGL2 renderer + WasmClock / WasmFontLoader /
                              #   WasmCursor + standard capability backends
                              #   (WasmClipboard / WasmHttp / WasmFilePicker). NO
                              #   #[wasm_bindgen] surface, NO playground code.
                              #   Exposes WasmAppHandle::create(WasmAppConfig)
                              #   (a builder with a `configure` callback for
                              #   extra plugins + an optional after-frame hook).
                              #   The host cdylib (demo/website/native) wraps it.
    tur-integration-tests/     # integration test harness + cases
    tur-demo-plugin/           # playground-only plugin (swc compiler services
                              #   only — file IO now lives in tur:filepicker)
    tur-native/                # native (non-wasm) embedder entry point
    tur-filepicker-capability/ # FilePicker capability + FilePickerBackend trait
                              #   + tur:filepicker bridge (exports `filePicker`
                              #   { pick, saveFile }). Opt-in: requires a real
                              #   backend (no no-op default).
    tur-filepicker-wasm/       # WasmFilePicker backend (web-sys <input type=file>
                              #   + <a download>).
    tur-filepicker-native/     # NativeFilePicker backend (rfd async dialog).
 integrations/
   compose/                    # Pure-Kotlin Compose AAR (`org.tur`): TurView +
                              #   TurRuntime + TurInstance + FrameLoop + InputMapper + TurNative
                              #   (external-fun bridge). Ships NO .so — accepts
                              #   a runtime handle via TurRuntimeFactory (the app
                              #   loads its own .so and builds the engine).
 demo/
   compose/                    # Android playground app: MainActivity + DemoNative
                              #   (loads libtur_demo.so, declares createEngine) +
                              #   the gradle cargo-ndk pipeline
      native/                   # `tur-demo` cdylib crate: the app's .so. Links
                              #   tur-android (rlib) + standard_jni_exports!() +
                              #   a createEngine fn with the demo plugin set
                              #   (Std+Animation+Clipboard+Net+FilePicker+
                              #   DemoHelper). The template users copy for their
                              #   own app's .so.
   website/                    # Web host app (@tur-ng/website): thin browser
                              #   wrapper that loads the wasm + the
                              #   playground-view bundle. Its rspack runs
                              #   wasm-pack on `native/` + bundles
                              #   playground-view's dist/impl.js.
      native/                   # `tur-website` cdylib: the host .wasm. Wraps
                              #   tur-wasm's WasmAppHandle + adds TurDemoPlugin
                              #   (swc compiler only). File IO lives in
                              #   tur:filepicker (registered by tur-wasm).
                              #   Exports #[wasm_bindgen] TurWebsiteApp
                              #   (create / create_in / loadAndRunModule /
                              #   dev_tool) → tur_website.js. The wasm mirror
                              #   of demo/compose/native.
   playground-view/            # @tur-ng/playground-view: the playground UI
                              #   bundle built with tur:animation + tur:std
                              #   (Sidebar/Editor/Viewer) + inlined case
                              #   sources. A reusable view the website
                              #   renders. Owns the playground-only cases
                              #   (cases/ — compiler-bridge-demo, github-viewer)
                              #   + the folded-in @tur-ng/demo-helper types.
 js/
   packages/
     tur-animation/            # Ambient TS types for `tur:animation`
                              #   (runtime provided by tur-animation crate)
     tur-core/                # Ambient TS types for `tur:core`
                              #   (engine-owned reactive primitives)
     tur-test-cases/          # Test cases (cases/, ~60 cases) — pure
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

**Workflow (TDD):** for engine bug fixes and behavior changes, write a failing ("red") test under `libs/tur-integration-tests/tests/` that pins the intended behavior **first**; confirm it fails on the current code, then implement the change until it passes ("green"). This catches regressions and clarifies intent before implementation. Use `cargo test --workspace --test element <name>` for the red→green cycle, then run the full suite (`cargo test --workspace --test element`) + clippy to confirm no regressions. Tests that assert on the engine's per-frame outcome can use `app.pump()` (returns `FrameOutcome { rendered, schedule }`) instead of `settle()`/`advance()` when they need to inspect the schedule decision.

### tur-website (wasm)

The website's wasm entry is `demo/website/native` (a cdylib that wraps the pure `tur-wasm` rlib). Build it directly:

```sh
cd demo/website/native && wasm-pack build --target web
cargo clippy --target wasm32-unknown-unknown -p tur-wasm -p tur-website -- -D warnings
```

### website (web host)

The website is the browser host app. Building it automatically runs `wasm-pack` (on `demo/website/native`), builds the playground-view bundle, and copies WASM assets + the compiled `impl.js` into the output:

```sh
cd js && pnpm build
cd demo/website && pnpm build
# Or use the rspack dev server
cd demo/website && pnpm dev
# → https://localhost:8080/ (self-signed cert)
```

The dev server always sets COOP/COEP headers (`Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-Embedder-Policy: require-corp` + `Cross-Origin-Resource-Policy: same-origin`) — the wasm multi-threaded backend uses `SharedArrayBuffer` + Web Workers via `wasm_thread`, which requires `self.crossOriginIsolated`. Without these headers `Worker.postMessage` panics with `DataCloneError: SharedArrayBuffer transfer requires self.crossOriginIsolated`. COEP value must be `require-corp`, NOT `credentialless` — `credentialless` is Chromium-only (Firefox desktop + Android never implemented it and silently ignore it, so `crossOriginIsolated` stays false).

### JS (js/ workspace — also covers demo/website + demo/playground-view)

```sh
pnpm install
pnpm build            # build all packages (incl. website + playground-view)
pnpm lint             # biome lint across js/ + demo/website + demo/playground-view
```

### Per-package JS builds

```sh
cd demo/playground-view && pnpm build
cd js/packages/tur-test-cases && pnpm build
```

### Android (on-device debug)

Android build (`cargo ndk` + `gradlew assembleRelease`), the unsigned-APK debug-sign flow (`apksigner`, `INSTALL_PARSE_FAILED_NO_CERTIFICATES`), install/launch, and the **macOS Sequoia `adb` local-network block** workaround (Apple-signed `python3` localhost proxy — Sequoia silently denies the third-party `adb` daemon LAN access with `No route to host`) live in the **`android-dev` skill** at `.opencode/skills/android-dev/SKILL.md`. Load it (`@android-dev`) whenever working with `tur-android` / `demo/compose` on a device or emulator.

## Conventions

- Rust edition 2024, MSRV 1.91
- JS: TypeScript strict mode, ESNext modules, rspack bundling
- Linting: biome
- Layout: Flutter-inspired (Column, Row, Expanded, Stack, Positioned). The layout model follows Flutter's flex layout — Column/Row are flex containers, Expanded fills remaining space, Container with explicit width/height constrains to those dimensions. Default cross-axis alignment for both Column and Row is `Center` (matching Flutter's behavior).
- Rendering: vello-hybrid (hybrid CPU/GPU sparse-strips vector graphics). Two backends: **WebGL2** (`WebGlVelloRenderer`, used by `tur-wasm` — native browser WebGL2, no wgpu dependency, ~3MB smaller binary) and **wgpu** (`VelloRenderer`, used by native integration tests — Vulkan/Metal/DX12/WebGPU). The `renderer/vello` module keeps the historical name. Shared `VelloPaintContext` + `scene_paint` helpers paint the element tree into a vello-hybrid `Scene`; each backend wraps it with its own renderer + `Renderer` trait impl. Backend selection is via tur-engine features: `wgpu-backend` (default, native) vs `webgl` (wasm). Also a noop renderer (logs tree stats).
- JS engine: boa_engine (pure Rust, compiles to wasm32)
- No separate RenderTree — layout and paint happen directly on ElementTree
- When developing, especially writing demo cases, if an engine-level issue is found, investigate and plan to fix it in the engine rather than working around it in the demo case itself.
- Publishable npm packages (the `@tur-ng/*` packages under `js/packages/` that carry a `publishConfig` block — `tur-core`, `tur-std`, `tur-animation`, `tur-clipboard`, `tur-net`): whenever you modify one, bump its **patch** version by exactly +1 over the latest version **published on the npm registry** (`npm view <pkg-name> version`, e.g. `npm view @tur-ng/std version` → set the branch's `version` to that + one patch). Do **not** use `main`'s `version` as the baseline — `main` frequently drifts ahead of the registry (e.g. `main` holds `0.1.0` while `@tur-ng/std` is published at `0.0.2`), so it yields wrong numbers. Never bump twice on the same branch for the same change; never bump minor/major unless asked.

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
cd demo/website && pnpm dev
# → https://localhost:8080/ (self-signed cert)
```

The dev server runs over HTTPS with a self-signed cert. Playwright MCP's default context rejects the cert — **bypass it** by opening a fresh context with `ignoreHTTPSErrors: true` via `playwright_browser_run_code_unsafe`:

```js
async (page) => {
  const newCtx = await page.context().browser().newContext({ ignoreHTTPSErrors: true });
  const newPage = await newCtx.newPage();
  await newPage.goto('https://localhost:8080/', { waitUntil: 'load' });
  // ...interact via newPage (the MCP snapshot tools won't see it — use evaluate / screenshot)
}
```

The new context's page isn't tracked by the MCP snapshot/click tools — use `newPage.evaluate(...)`, `newPage.screenshot(...)`, `newPage.on('console' | 'pageerror', ...)` directly inside the `run_code_unsafe` callback.

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


