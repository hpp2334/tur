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
│   globalThis.tur.widget.*)                           │
└──────────────────────┬──────────────────────────────┘
                       │ JS bridge API
┌──────────────────────▼──────────────────────────────┐
│  libs/tur (facade crate, generic over Renderer)      │
│  ├── libs/tur-boajs          (boa_engine JS bridge)  │
│  ├── libs/tur-widget         (widget types & layout)  │
│  ├── libs/tur-vello-renderer (vello painting backend) │
│  └── libs/tur-noop-renderer  (debug/logging backend)  │
└─────────────────────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│  libs/tur-wasm                                        │
│  (wasm binary via wasm-pack: winit + boajs + vello)  │
└─────────────────────────────────────────────────────┘
```

### Widget types

`Column`, `Row`, `Expanded`, `Stack`, `Positioned`, `SizedBox`, `Container`, `Text`

Flutter-like layout model: flex-based Column/Row with Expanded children, Stack with Positioned children.

## Directory structure

```
libs/
  tur/                    # Facade crate (re-exports all sub-crates)
  tur-widget/             # Widget types, layout algorithms
  tur-vello-renderer/     # Vello painting backend
  tur-noop-renderer/      # Debug/logging backend (implements Renderer trait)
  tur-boajs/              # boa_engine JS bridge (globalThis.tur.widget.*)
  tur-wasm/               # wasm binary (winit + boajs + vello)
js/
  packages/
    tur-solidjs-renderer/ # SolidJS universal renderer
    tur-solidjs-demo/     # Demo app (todolist example)
    tur-wasm-cli/         # CLI for building and serving tur-wasm demos
```

## Commands

### Rust (workspace root)

```sh
cargo build --workspace
cargo test --workspace
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

### tur-wasm-cli (serve demos)

```sh
# Build JS bundle first
cd js && pnpm build
# Serve a JS demo
node js/packages/tur-wasm-cli/bin/cli.cjs serve <path-to-bundle.js>
```

Or use the convenience script:

```sh
node scripts/serve-web-demo.cjs
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

### Renderer trait

The `Renderer` trait is defined in `tur-render-tree`:

```rust
pub trait Renderer {
    fn render(&mut self, tree: &RenderTree);
}
```

`TurApp<R: Renderer>` is generic over the rendering backend. Use `VelloRenderer` for GPU rendering or `NoopRenderer` for debug logging.
