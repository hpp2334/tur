# tur

A JavaScript-free rendering engine built on vello-hybrid and the **rut** scripting VM
(rut-lexer / rut-parser / rut-vm). Modules are **rut** sources: they call into the
engine through the `tur_host` pkg rows registered by engine plugins.

## Roleplay

You are a professional software engineer.

## Requirement

Clean design and architecture.

## Module lifecycle contract

`load_rut_module` (on the app handle; `TurApp::load_rut_module_source` is the
handle-based twin) is the ONLY module entry (plus `load_rut_bundle` in the test
harness). A loaded module MUST export `entry fn start()`:

- The engine parses the new module FIRST (a broken reload never destroys the
  running module's tree — the parse error is reported to the embedder), then
  runs the previous module's `entry fn stop()` (if present) and clears any
  leftover root tree (draining its `before_destroy` lifecycle), then boots the
  new module and invokes `start`. `start` may return a `u64` answer — the
  module's return value, readable via `TurApp::rut_start_answer` (the standard
  probe channel: modules return their label/binding atom ids).
- The root-tree lifecycle is ENGINE-OWNED: `tur_host::mount(view)` stashes the root
  (a pure-Rust `Rc<dyn View>` — no script runtime anywhere in the tree); the
  engine applies it and module teardown clears it. A module's `stop` only
  disposes its own non-tree resources.
- Cleanup also runs (best-effort) at instance destroy.
- The script runtime is the rut VM: it is driven by the worker pump
  (`run_ready()` before each flush — never inside a flush iteration). The VM's
  virtual clock syncs to the engine clock; task traps surface through the
  runtime-error rail (`HostMsg::RuntimeError`) — never a silent stall.
- Fuel/heap: the VM is arity- and type-checked at the row boundary (row decls
  are typed: `u64` / `f64` / `str` / `bytes` / `bool` / `opaque`); a row call
  mismatching its declaration is a compile-time parse error, not a runtime
  surprise.

Entry points follow the contract: the test corpus
(`rut/cases` + the playground-local cases) authors
`entry fn start()` that builds its tree through the **kit** (`use
tur_kit::{ Column, Text, … }` — see Conventions) and hands the root to the
engine with `mount(root.build())`, plus probe `entry fn`s the test
drives via `call_rut_entry`. The playground is a rut module
(`playground.rut` + the generated `cases_gen.rut` registry, compiled by the
`pg_compile` rut service); the website boots it via `loadAndRunRutModule`.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  demo/website (@tur-ng/website)                     │
│  Thin browser host: loads the tur WASM and feeds    │
│  playgroundSource() to loadAndRunRutModule.         │
│  Co-located with its own wasm cdylib                │
│  (demo/website/native → tur-website).               │
├─────────────────────────────────────────────────────┤
│  rut modules (playground.rut + the case corpus)     │
│  authored through the kit prelude (tur_kit — the    │
│  builder classes); element builders materialize     │
│  pure-Rust views.                                   │
└──────────────────────┬──────────────────────────────┘
                       │ rut host-pkg rows (`tur_host::…`)
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-engine (unified engine crate)             │
│  core/        engine infrastructure (no plugin deps)│
│  rut_runtime/ the `tur_host` pkg mechanism: RutView,│
│               RutHandles, HostPkg wiring, mount,    │
│               the rs_* store rows, entry rails      │
│  builtin_plugins/ feature bundles — subsystems +    │
│               their OWN family rows + rut_rows.rs   │
│               (text, scroll, gesture, image,        │
│               virtual_app …); each element family's │
│               spec struct + rows live here          │
│  kit/         registers the kit pkg (rut/tur_kit/   │
│               — the authored builder surface);      │
│               outside core/                         │
│  renderer/vello WebGL2 + wgpu backends              │
├─────────────────────────────────────────────────────┤
│  libs/tur-animation (standalone crate)              │
│  AnimationSubsystem + the `tur_host` pkg's animation│
│  rows (anim_ctrl / el_opacity / tween / curve) —    │
│  Rust-held controllers ticked by the subsystem.     │
├─────────────────────────────────────────────────────┤
│  Capability surfaces: Clipboard / Http / FilePicker │
│  (backend crates per platform) + their `tur_host` pkg│
│  rows (clipboard_read / net_request / pick_file).   │
├─────────────────────────────────────────────────────┤
│  libs/tur-wasm (pure rlib — the reusable wasm       │
│  embedder) + demo/website/native (the host cdylib). │
│  libs/tur-android (rlib embedder glue) +            │
│  demo/compose (the Android playground app).         │
└─────────────────────────────────────────────────────┘
```

### Capability registry

Embedders register swappable backends (clipboard, http, filepicker) on the
runtime builder: `.capability(|cx| Ok(Http::new(backend)))`. Plugins declare
hard deps via `requires` (fail-fast at `build()`); `TurNetPlugin` is the
exception — it feature-detects `Http` at `register` and skips pushing the net
rows when absent. The `tur_host` pkg rows reach capabilities at call time through
`RutHandles.inst.capability()`.

### Reactive substrate

`core::edgy` — the reactive substrate is native: the store KV holds `Value`
(scalars / strings / lists / maps / opaque handles), sources / derives /
mutations are atom ids over one per-instance counter, and rut modules address
them by raw id (the `rs_*` rows: `rs_source_str` / `rs_set_f64` /
`rs_derive` / `rs_watch` …). Rust plugins mint atoms via
`PluginRegisterContext::reactive()` (`build_derive` / `build_mutate` — Rust
closures over the read/write faces). Engine environment atoms
(`viewportSize$`) follow the same rail (a backing source + a derive handle).

### Multi-instance model (TurRuntime + TurApp)

One runtime, many instances (unchanged from the JS era except every instance
hosts a rut VM instead of a JS realm): `TurRuntime` owns fonts / clock /
capabilities / plugins; `app_builder()…build()` spawns an isolated instance
(own rut VM, tree, store, focus manager, subsystems) onto a worker lane;
`build_headless` for no-render instances. `VirtualAppView` hosts nested
instances (child = a full engine instance; its batch replays into the
parent's). Pinned by `tests/element/multi_instance.rs`,
`worker_pool.rs`, `worker_spawn_blocking.rs`.

### Element types

`Column`, `Row`, `Expanded`, `Flexible`, `Stack`, `Positioned`, `SizedBox`,
`Container`, `Grid`, `Table`, `PointerInteract`, `MouseRegion`, `Focusable`,
`Text`, `Input`, `Paragraph`, `Image`, `Svg`, `VirtualAppView`,
`Opacity`, `Transform` (animation rows), `ScrollView`, `Scrollbar`,
`LazyList`, `LazyGrid`, `Condition`, `Switch`, `Each`, `Fragment`.

### Flutter-like layout model

Unchanged: flex Column/Row with Expanded/Flexible (tight/loose), Stack +
Positioned, Container sizing, degenerate-case degradation with one error log,
ScrollView = SingleChildScrollView semantics, LazyList/LazyGrid = ListView
semantics, Flutter-parity hit testing (`HitTestSelf`), render dedup via batch
fingerprints.

### Rendering

vello-hybrid, two backends: WebGL2 (`WebGlVelloRenderer`, wasm) and wgpu
(`VelloRenderer`, native) + a noop renderer. The worker records a
`RenderCommandBatch`; the host applies + presents it at the render commit
point (frame-deduped by content fingerprint).

### Module runtime

The rut VM (rut-vm): compiled modules run as tasks with a virtual clock
synced to the engine clock (`run_ready` at pump level). Async engine APIs
(clipboard / net / filepicker) are `pkg_async_fn!` rows returning
`Completer`s — rut code `await`s them through the `async_host` weave
(`launch_future` + `await`). The engine's derive/watch rows (`rs_derive`,
`rs_watch`) call back into the VM through the guarded sync face during flush
(never while the flush holds tree borrows; a mount attempt inside a derive
traps and reports through the error rail).

### Debugging the playground

The whole playground renders to a single `<canvas>` — tur renders its own UI.
The dev tool is engine-native: `TurApp::dev_tool_element_tree` /
`dev_tool_get_element` / `dev_tool_frame_stats` / `set_host_frame_timing`
(serialize on the worker — see `core::dev` — and surface as the page-level
`turDevTool` global's methods returning JSON strings). Drive the browser with
the `agent-browser` CLI; delegate seeing + canvas input to the operator
subagent. `elementTree()` shapes are unchanged from the JS era.

## Directory structure

```
libs/
  tur-engine/          # unified engine crate (core + builtin_plugins +
                       #   kit/ + renderer/vello + rut_runtime)
  tur-animation/       # animation subsystem + `tur_host` pkg animation rows
                       #   (+ rut/tur_anim_kit/ — the Opacity/Transform wrappers)
  tur-clipboard-*/     # capability + wasm/native/android backends
  tur-net-*/           # capability + wasm/native backends
  tur-filepicker-*/    # capability + wasm/native backends
  tur-native/          # native-only platform integrations (fonts, worker pools)
  tur-wasm/            # the reusable wasm embedder rlib
  tur-android/         # the Android embedder glue rlib
  tur-integration-tests/ # harness + integration corpus
demo/
  website/             # the web host app + native/ (tur-website cdylib)
  compose/             # Android playground app + native/ (tur-demo cdylib)
rut/                   # all rut package sources (pure rut — no build step)
  cases/               # the shared case corpus (<name>/index.rut + README)
  tur_kit/             # the authored builder surface (tur_kit.rut; embedded
                       #   by tur-engine/src/kit/mod.rs)
  tur_anim_kit/        # the animation kit (kit.rut; embedded by
                       #   tur-animation/src/kit.rs)
  tur_net_kit/         # the net kit (kit.rut; embedded by
                       #   tur-net-capability/src/kit.rs)
  playground/          # playground.rut + cases_gen.rut + showcase.json +
                       #   scripts/gen-cases.cjs
```

## Commands

### Rust (workspace root)

```sh
cargo build --workspace
cargo nextest run --workspace            # test runner: per-test process isolation
cargo test --workspace --doc --locked    # doctests
cargo clippy --workspace -- -D warnings
cargo clippy --target wasm32-unknown-unknown -p tur-wasm -p tur-website -- -D warnings
```

Tests run under cargo-nextest (`.config/nextest.toml`; CI uses `--profile ci`
with retries = 2). The rut case corpus needs no build step — cases are plain
`.rut` sources read from disk (`load_rut_bundle` / the website embeds them at
compile time).

**Workflow (TDD):** for engine bug fixes, write a failing test under
`libs/tur-integration-tests/tests/` first (rut fixtures — see the corpus
README for the fixture conventions), confirm red, then make it green.

### tur-website (wasm)

```sh
cd demo/website/native && wasm-pack build --target web
cd demo/website && pnpm build      # bundles + copies the wasm
cd demo/website && pnpm dev        # → http://localhost:8080/
cd demo/website && pnpm dev:tunnel # → https://local-tur.hpp2334.com (cloudflared `tur-local`)
```

The dev server sets COOP/COEP headers (required by the multithreaded wasm
backend — `SharedArrayBuffer` + workers; COEP must be `require-corp`).
`dev:tunnel` serves the playground over a public custom domain via the
`cloudflared.yml` next to the config (one-time `cloudflared tunnel login` /
`create` / `route dns` setup — see that file's header).

### JS tooling (repo root)

The pnpm workspace + lint tooling live at the repo root (`demo/website` is
the only workspace package; biome lint + the website build run from root).

```sh
pnpm install
pnpm lint
```

### Android

Android build + device debugging live in the **`android-dev` skill** at
`.opencode/skills/android-dev/SKILL.md`.

## Conventions

- Rust edition 2024, MSRV 1.91
- Layout: Flutter-inspired (see the layout notes above; unchanged).
- **Host-pkg rows are the ONLY script surface**: every `tur_host::…` call is a
  typed row — a `pkg_fn!`/`pkg_async_fn!` body + a `decl_rows` entry. The
  decl (compile-time) and body (runtime) signatures must agree exactly.
- **The kit is THE element construction surface** (`rut/tur_kit/tur_kit.rut`,
  animation wrappers in `rut/tur_anim_kit/kit.rut`):
  one wrapper CLASS per element over its family's rows — chainable,
  ONE METHOD PER PROP, names = the historical camelCase props in rut
  snake_case (`cross_alignment`, `query_key`, `item_builder`, `font_size`,
  `obscure`, …). Construction is the class call form `X()` over the
  `[constructor] fn builder()` (`Row()`, `SizedBox(400.0, 200.0)`) — the
  `.builder()` long form stays valid (byte-identical lowering) — and
  construction is CHAINED, never statement-mutated
  (`let c = X().prop(..).child(..);`, not `let mut c` + `c.prop(..)` runs).
  `.child(c)` / `.children([…])` append children; `.build()`
  is the ONLY terminal and calls the FAMILY's build row (`Column.build()` →
  `flex_build`, `Text.build()` → `text_build`). Required-prop validation
  stays in the rows/View constructors. **Handlers are mutations**
  (`on_click(mutate(fn (ctx: MutationCtx) { … }))` over the MutationCtx —
  the boa triad: source / derive / mutate; writes flow through `ctx.set_*`,
  composition through `ctx.run*`, state by capture); fn-values are the
  SUBSTRATE (the branch builders stay plain fn literals —
  `Each(items).item_builder(fn (i: u64, item: str) -> View { … })` —
  sealed into opaque boxes rut-side, fired through the infra dispatch
  entries); the arity/type check happens at the kit boundary at compile
  time. Nothing callable ever crosses the boundary as a string or a
  closure. Case modules declare `entry fn` ONLY for `start` (+ deliberate
  test/embedder probes — `entry` = a published contract, never a
  callback). Async journeys are PLAIN RUT: `launch_future(work(handles…))`
  over ordinary params — the launching mutation hands its own
  `MutationCtx` along as a plain value when the body needs access (a
  launch site with no ctx of its own — `start`, the boot rail — mints
  the entry rail's ctx, `entry_ctx()`); `await sleep` rides the futures
  prelude. No `TaskCtx`/`spawn`/`Task`; the stash rails (`st_*` /
  `stf_*`) are DELETED (state crosses as `AppContext` fields — see the
  fixture contract below). Reactive bindings are methods, not variants:
  the literal keeps the base
  prop (`Text().text("hi")`, `Container().color(0x…u64)`), the reactive
  lane is the `*_bound` method over `Readable<T>` (`Text().text_bound(r)`,
  `Container().color_bound(r)`, `Expanded().flex_bound(r)`). The
  unified one-name-over-`Readable<T>` law (base prop takes the handle,
  `sv`/sugar for literals, the `*_bound` twins gone) is blocked on
  param-type overloads — rut at this pin rejects duplicate fn/method
  names outright; it lands with the compiler feature. Flags are per-family
  NAME-ONLY enums on the kit (`Align`, `MainAlign`, `CrossAlign`,
  `MainAxisSize`, `StackFit`, `Axis`, `BoxFit`, `BorderPosition`, `Clip`,
  `HitTestBehavior`, `SpanFlags`, `Cursor`): kit methods take the enum and
  unwrap once at the row (`flex_main_align(self.spec, code(v))`) — the
  mappers are the sole carriers of the row codes (the `tur_host` u64
  const pushes are deleted). The kit hides row churn from call sites;
  the rows are the boundary.
- **The layering law**: `core/` owns MECHANISM, never elements. Zero
  references to `builtin_plugins`, zero element/view names, no shared builder
  contract (no `RutBuilder` trait, no generic `el_build`/`el_child`/`el_qkey`
  rows). Every element family is complete unto itself, owned by the plugin
  that owns its view type: its spec struct, its constructor row, its setter
  rows, its `*_child` row, its `*_qkey` row, and its own `*_build` terminal
  (family-prefixed names: `flex_*`, `box_*`, `text_*`, `input_*`, `stack_*`,
  `pos_*`, `scroll_*`, `lazy_*`, `each_*`, `cond_*`, `switch_*`, `pi_*`,
  `mr_*`, `focus_*`, `img_*`, `va_*`, …) — each installed via its own
  `rut_rows.rs` + `push_rut_ext` (the tur-animation installer pattern). The
  kit lives OUTSIDE `core/` and is registered by the standard plugin set
  (`TurStdPlugin` prelude), never by core. Pinned by
  `libs/tur-integration-tests/tests/layering.rs`.
- Module fixtures in tests: `load_rut_module` (inline) / `load_rut_bundle`
  (corpus); state probes are `entry fn`s, bound labels (query keys),
  dev-tool tree queries, or controller rows — never a script realm poke.
- **The context-crossing fixture contract** (inline test fixtures): the
  module builds an `AppContext` record and the EMBEDDER holds it between
  entry calls — no stash, no raw-atom args. `fn start() -> AppContext` +
  `entry fn entry_start() -> opaque` boxing it (boot defers to the
  embedder's first probe), OR the eager twin `entry fn start() -> opaque`
  (the engine boots it at load; the slot token IS the
  `rut_start_answer`). Control entries take `cx: opaque` and downcast —
  `opaque.downcast<AppContext>(cx)` behind a shared `*_cx(cx)` helper
  (the nil-guard is belt-and-braces; a kind mismatch is the loud
  channel) — and writes ride `entry_ctx()` (the entry rail's ctx).
  Harness: `call_rut_entry_opaque("entry_start")` → the context token;
  `call_rut_entry_cx` / `_cx_u64` / `_cx_f64` / `_cx_str` drive the
  control entries. Mirrors become `cx` fields or answered values (a
  `-> str` probe). Real-input driving (clicks/keys) stays for genuine
  gesture tests. The corpus CASES keep `entry fn start() -> u64` +
  atom-arg probe entries (the playground embeds them; their boot must be
  eager) — their ctx hatch is the kit's `bridge()`/`over` probe rail.
- Known upstream rut bug (pin 80b56c6, the duplicate-boot-scope
  type-interning family): some module-shape edits trip it — kit fn/row
  resolution collapses ("argument N is X, Y expected" far from the edit)
  or an interface-typed capture misbinds at runtime ("no impl for
  interface slot N"). Accommodations that hold: keep captured sources
  CONCRETE-annotated (`Source<T>`, not `Readable<T>`) when they cross
  ctx calls or struct fields; make import-list deltas single-name; an
  interface-annotated `let` before the first `mutate` seeds the table;
  the playground keeps a second `use tur_host` line (load-bearing). The
  playground's status label spells its derive through `rs_derive` (the
  plain-fn rail) for the same reason.
- Linting: biome (root workspace; `pnpm lint` at the repo root).
- Publishable npm packages: none — the `@tur-ng/*` packages and the `js/`
  workspace are gone; `@tur-ng/website` remains (the private website shell).
- Async: capability rows are async functions awaited through the rut weave;
  cancel rides the task opaque's cancel row (`net_stream` + `task_cancel`).
- Runtime errors: VM task traps + face traps ship as
  `HostMsg::RuntimeError{report}` to the embedder (see
  `core::app::runtime_error`); the virtual-app status rail reports child
  boot/runtime failures through `va_status` / `va_error`.

## Renderer trait

Unchanged (see `tur-engine::core::render`): `render_commands` + `present` +
`resize` + `upload_image_resource` + `render_to_pixels`.

## Debugging the playground (main agent + operator)

Start the dev server (`cd demo/website && pnpm dev` →
http://localhost:8080/ — open with `agent-browser open
http://localhost:8080/`), drive the canvas via
`agent-browser mouse/eval/press` + `turDevTool.elementTree()` (JSON — the
shapes are unchanged), verify colors by sampling pixels, and shut the server
down afterwards (`lsof -ti:8080 | xargs kill`, `rm -rf .agent-browser`).
Screenshots go under the gitignored `.agent-browser/` and are never committed.

## Invoking the git-end subagent

When the user asks to commit/push/PR (e.g. `@git-end`), dispatch the
**git-end** subagent with the fixed prompt `You are git-end agent.` — its
workflow lives in `.opencode/agents/git-end.md`.
