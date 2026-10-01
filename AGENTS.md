# tur

A JavaScript-free rendering engine built on vello-hybrid and the **rut** scripting VM
(rut-lexer / rut-parser / rut-vm). Modules are **rut** sources: they call into the
engine through the `tur` host pkg rows registered by engine plugins.

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
- The root-tree lifecycle is ENGINE-OWNED: `tur::mount(view)` stashes the root
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
(`js/packages/tur-test-cases/cases` + the playground-local cases) authors
`entry fn start()` that mounts its tree through the `tur` host pkg rows
(`el_column` / `el_text_bound` / `mount` …), plus probe `entry fn`s the test
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
│  authored against the `tur` host pkg rows —         │
│  element builders materialize pure-Rust views.      │
└──────────────────────┬──────────────────────────────┘
                       │ rut host-pkg rows (`tur::…`)
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-engine (unified engine crate)             │
│  core/        engine infrastructure (no plugin deps)│
│  rut_runtime/ the `tur` host pkg: decl rows (the    │
│               compile-time surface) + bodies        │
│               (per-instance HostPkg closures over   │
│               RutHandles: store / tree / intents)   │
│  builtin_plugins/ feature bundles — subsystems +    │
│               pkg-row installers (text, scroll,     │
│               gesture, image, virtual_app …)        │
│  renderer/vello WebGL2 + wgpu backends              │
├─────────────────────────────────────────────────────┤
│  libs/tur-animation (standalone crate)              │
│  AnimationSubsystem + the `tur` pkg's animation     │
│  rows (anim_ctrl / el_opacity / tween / curve) —    │
│  Rust-held controllers ticked by the subsystem.     │
├─────────────────────────────────────────────────────┤
│  Capability surfaces: Clipboard / Http / FilePicker │
│  (backend crates per platform) + their `tur` pkg    │
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
rows when absent. The `tur` pkg rows reach capabilities at call time through
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
                       #   renderer/vello + rut_runtime)
  tur-animation/       # animation subsystem + `tur` pkg animation rows
  tur-clipboard-*/     # capability + wasm/native/android backends
  tur-net-*/           # capability + wasm/native backends
  tur-filepicker-*/    # capability + wasm/native backends
  tur-native/          # native-only platform integrations (fonts, worker pools)
  tur-wasm/            # the reusable wasm embedder rlib
  tur-android/         # the Android embedder glue rlib
  tur-integration-tests/ # harness + integration corpus
demo/
  tur-playground-…     # (retired — the swc plugin is gone)
  website/             # the web host app + native/ (tur-website cdylib)
  compose/             # Android playground app + native/ (tur-demo cdylib)
js/
  packages/
    tur-test-cases/    # the rut case corpus (cases/*/index.rut)
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

### JS

The js workspace now only carries lint tooling (biome) + the rut corpus. No
per-package builds remain.

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
- **Host-pkg rows are the ONLY script surface**: every `tur::…` call is a
  typed row — a `pkg_fn!`/`pkg_async_fn!` body + a `decl_rows` entry. The
  decl (compile-time) and body (runtime) signatures must agree exactly.
- Module fixtures in tests: `load_rut_module` (inline) / `load_rut_bundle`
  (corpus); state probes are `entry fn`s (`call_rut_entry`), bound labels
  (query keys), dev-tool tree queries, or controller rows — never a script
  realm poke.
- Linting: biome (the js workspace is lint-only).
- Publishable npm packages: the `@tur-ng/*` packages are gone from `js/`;
  `@tur-ng/website` remains (the website shell).
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
