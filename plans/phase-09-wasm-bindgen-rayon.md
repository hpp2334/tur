# Phase 9 — Wasm threads via `wasm-bindgen-rayon`

**Status:** not started
**Prerequisite:** Phase 8 (native fully migrated; architecture proven).
**Goal:** bring wasm online with the same `WorkerPool` (Model B) as native, using `wasm-bindgen-rayon` for real `std::thread`-equivalent semantics on top of SharedArrayBuffer + atomics. Production deployment serves COOP/COEP headers permanently (decided earlier — wasm is demo-only for now, so deployment constraints are acceptable).

## Background

With `+atomics,+bulk-memory` enabled on the wasm32 target, `wasm-bindgen-rayon` provides a `spawn_blocking`-equivalent backed by Web Workers + SAB. The engine's `WorkerPool` code from Phase 7 then works identically on wasm — `handle.spawn_blocking(worker_loop)` runs `worker_loop` on a wasm worker thread.

Benefits:
- Same Model B pool as native (one codepath).
- Real shared memory via SAB — channel messages are pointer passes, no serialization.
- Per-instance boa Context still bound to one worker (boa is `!Send` even under `+atomics`).

## Phase 0 verification (do this first, before committing to Phase 9)

Validate the dep tree compiles with `+atomics`:

```sh
RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals" \
  cargo check --target wasm32-unknown-unknown -p tur-engine
```

Suspected-problematic deps:
- `boa_engine` (pinned git rev `6ef5370c…`) — pure Rust, no `js-sys`/`web-sys` in core; likely fine.
- `vello_hybrid`, `parley`, `peniko` — pure Rust; likely fine.
- `reqwest-wasm` — already wasm-target; should compile under `+atomics`.
- `swc` (tur-demo-plugin) — large pure-Rust dep tree; likely fine but slow.

If any dep fails, document the blocker and either (a) work around it (patch / fork), or (b) fall back to a separate worker wasm module (postMessage protocol — significantly more work, deferred).

## Changes

### 9.1 — `Cargo.toml` workspace deps

```toml
[workspace.dependencies]
wasm-bindgen-rayon = "1.2"
```

Add to `libs/tur-engine/Cargo.toml`:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-rayon = { workspace = true }
```

### 9.2 — `.cargo/config.toml` (new file in workspace root)

```toml
[target.wasm32-unknown-unknown]
rustflags = [
    "-C", "target-feature=+atomics,+bulk-memory,+mutable-globals",
    "-C", "link-arg=--shared-memory",
]
```

This applies the atomics flags to every `wasm32-unknown-unknown` build automatically.

### 9.3 — `tur-wasm/src/lib.rs` — init thread pool

```rust
#[cfg(target_arch = "wasm32")]
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    wasm_bindgen_rayon::init_thread_pool_once();
}
```

`init_thread_pool_once` lazily builds the underlying pthreads-equivalent pool on first use. Must be called once before any `spawn_blocking`.

### 9.4 — `tur-wasm` tokio runtime

`wasm-bindgen-rayon` doesn't provide a tokio runtime; tokio itself doesn't run on wasm32 with multi-threading today. Two options:

**Option A — provide a wasm-specific `Handle` shim:**
- The engine's `WorkerPool` only uses `handle.spawn_blocking(...)`. Implement a custom `Handle`-like type for wasm that calls `wasm_bindgen_rayon::spawn(...)`.
- Engine's WorkerPool abstracts over `Handle` via a trait.

**Option B — use `tokio` with the `wasm-bindgen` feature under `+atomics`:**
- Newer tokio versions support wasm32 with atomics. Verify against the current pinned version.
- If supported, no engine changes — wasm gets the same tokio runtime as native.

**Recommendation: try Option B first.** If tokio doesn't cooperate, fall back to Option A.

### 9.5 — `demo/website/rspack.config.ts` — COOP/COEP permanently

Today (`rspack.config.ts:128-134`), COOP/COEP headers are gated behind `TUR_TUNNEL=1`. Remove the gate:

```ts
headers: {
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "credentialless",
    "Cache-Control": "no-store",
},
```

Now always set. Production deployment (whatever serves the website) must also set them.

### 9.6 — `wasm-pack` invocation

`demo/website/rspack.config.ts:21` runs `wasm-pack build --target web --no-opt`. With atomics, the invocation may need additional flags:

```sh
wasm-pack build --target web --no-opt -- \
    -C target-feature=+atomics,+bulk-memory,+mutable-globals
```

Or rely on `.cargo/config.toml` (preferred — applies uniformly).

The output `tur_website_bg.wasm` must be served with `Cross-Origin-Embedder-Policy: credentialless` (or `require-corp`) for SharedArrayBuffer to be available in the wasm.

### 9.7 — Worker thread initialization

Each wasm worker spawned by `wasm-bindgen-rayon` runs the same wasm module. The engine's worker_loop (from Phase 7) must be runnable from any thread. Specifically:
- `boa Context` construction is thread-local — fine.
- `tracing_wasm` / `console_error_panic_hook` — already set globally on main; worker threads inherit via the shared wasm module.
- DOM access (`tur-wasm` capabilities) — only main thread can touch DOM. Worker invokes capabilities via the channel (Phase 7's design).

### 9.8 — Clipboard / FilePicker proxying on wasm

Per the earlier audit, on wasm:
- `Http` (`reqwest-wasm`) — fetch is callable from workers. Stays on worker.
- `Clipboard` (`navigator.clipboard`) — main-thread-only (focus-dependent). Worker proxies via `MainMsg::ClipboardRequest` → main → reply.
- `FilePicker` (`<input type=file>`) — DOM-only. Worker proxies via main.

The proxy messages:
```rust
enum MainMsg {
    ...
    ClipboardReadRequest { reply: oneshot::Sender<Result<String, ...>> },
    ClipboardWriteRequest { text: String, reply: oneshot::Sender<Result<(), ...>> },
    FilePickerRequest { ... reply: oneshot::Sender<Result<...>> },
}
```

Main handles these via its main-side capability backends (which exist today).

## Verification

1. **Phase 0 check** — `RUSTFLAGS="-C target-feature=+atomics,..." cargo check --target wasm32-unknown-unknown` succeeds for the full workspace.
2. **COOP/COEP verify** — `demo/website && pnpm dev` serves headers; `crossOriginIsolated === true` in browser console.
3. **Off-main-thread verify** — log thread ID inside flush; differs from main.
4. **Playground smoke test** — full playground (sidebar, editor, viewer) renders correctly; compilation, run, scroll, text input work.
5. **Multi-instance on wasm** — verify two `TurWebsiteApp` instances share the worker pool.
6. `cargo build --target wasm32-unknown-unknown --workspace` clean (under atomics).
7. `cargo clippy --target wasm32-unknown-unknown -p tur-wasm -p tur-website -- -D warnings`.

## Risks

- **Dep compile failures under `+atomics`** — the biggest unknown. Could be a multi-day yak-shave if `boa`/`vello`/`swc` doesn't cooperate. Mitigation: validate in Phase 0 before any architecture work.
- **wasm-bindgen-rayon thread-pool init timing** — must happen before any spawn_blocking. The init takes ~100ms on first call (spawns N web workers, each loads the wasm). Lazy init may cause a startup stall. Pre-warm during app construction.
- **Tokio on wasm with atomics** — if Option B doesn't work, Option A adds a `Handle` trait abstraction. Plan a fallback path before starting Phase 9.
- **Browser support** — older Safari versions don't support SharedArrayBuffer even with COOP/COEP. Document the floor (Safari 16.4+, all evergreen Chrome/Firefox).
- **Build time** — `+atomics` adds compilation overhead. CI times may increase.

## Out of scope

- Removing the `direct-render` fallback — keep it as a feature flag for tests.
- Production hardening (browser matrix testing, perf tuning) — follow-up.

## Estimated scope

- ~150 lines new (init, config, possibly Handle shim)
- ~100 lines modified (rspack config, Cargo.toml, lib.rs)
- Major unknown: Phase 0 dep-compile validation (could be 1 day or 1 week)
- Single PR after Phase 0 passes
