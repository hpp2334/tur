# Phase 6 — `Send + Sync` prep (Capabilities + Plugins + tur-net-native)

**Status:** not started
**Prerequisite:** Phase 5 (escape hatches gone, channel-based API stable).
**Goal:** convert every `Rc`-shared cross-thread type to `Arc`, add `Send + Sync` supertraits to `Plugin` and capability backend traits, simplify `NativeHttp::new()` to pull the tokio `Handle` from the runtime context. Still single-threaded mode; this is purely "make the types Send-friendly so Phase 7 can move them across threads."

## Background

Today everything is `Rc<RefCell<>>` (single-threaded). For Phase 7's worker pool, the capability backends (Clipboard/Http/FilePicker) must be invocable from worker threads. That requires:

1. The capability newtypes (`Clipboard`, `Http`, `FilePicker`) to be `Send + Sync`.
2. The backend traits (`ClipboardBackend`, `HttpBackend`, `FilePickerBackend`) to require `Send + Sync`.
3. `Plugin: Send + Sync` so the same plugin objects can register into instances on multiple worker threads.
4. `NativeHttp::new()` drops its `Handle` parameter — the runtime supplies one.

These are mechanical changes — verified by the earlier audit that all six concrete plugins (`TurStdPlugin`, `TurAnimationPlugin`, `TurClipboardPlugin`, `TurNetPlugin`, `TurFilePickerPlugin`, `TurDemoPlugin`) are zero-field unit structs (trivially `Send + Sync`).

## Changes

### 6.1 — Capability handle types: `Rc` → `Arc`

| File | Today | Tomorrow |
|---|---|---|
| `builtin_plugins/clipboard/capability.rs:48` | `pub struct Clipboard(Rc<dyn ClipboardBackend>);` | `pub struct Clipboard(Arc<dyn ClipboardBackend + Send + Sync>);` |
| `core/platform/cursor.rs:194` | `pub struct CursorCap(Rc<RefCell<dyn CursorBackend>>);` | **Stays Rc** — cursor backend lives on main only |
| `tur-net-capability/src/lib.rs` | `pub struct Http(Arc<dyn HttpBackend>);` | Verify already `Arc` (likely yes); ensure `+ Send + Sync` bound |
| `tur-filepicker-capability/src/lib.rs` | `pub struct FilePicker(...)` | Same |

Cursor stays on main (renderer-thread-affine). Clipboard/Http/FilePicker move to worker (invoked from JS bridge).

### 6.2 — Backend traits: `Send + Sync`

```rust
// builtin_plugins/clipboard/capability.rs
pub trait ClipboardBackend: Send + Sync {
    fn read_text(&self) -> Pin<Box<dyn Future<Output = Result<String, ...>> + Send>>;
    fn write_text(&self, text: String) -> Pin<Box<dyn Future<Output = Result<(), ...>> + Send>>;
}

// tur-net-capability/src/lib.rs
pub trait HttpBackend: Send + Sync { ... }

// tur-filepicker-capability/src/lib.rs
pub trait FilePickerBackend: Send + Sync { ... }
```

All `Box<dyn Future>` returns must be `+ Send` (backends cross thread boundaries). Update existing backend impls:

| Crate | Status |
|---|---|
| `tur-clipboard-wasm` (`WasmClipboard`) | Already `Copy + Send + Sync` (unit struct); futures already `Send` |
| `tur-clipboard-native` (`NativeClipboard`) | Unit struct; `arboard` futures are sync — wrap in `Ready` (already done) |
| `tur-clipboard-android` (`AndroidClipboard`) | Holds `JavaVM` (Send + Sync — JVM is process-wide); verify |
| `tur-net-wasm` (`WasmHttp`) | Unit struct; reqwest-wasm futures — verify Send |
| `tur-net-native` (`NativeHttp`) | Holds `tokio::runtime::Handle`; reqwest futures — verify Send (was the canonical pattern) |
| `tur-filepicker-wasm` (`WasmFilePicker`) | Unit struct |
| `tur-filepicker-native` (`NativeFilePicker`) | Unit struct; rfd async — verify Send |

### 6.3 — `Plugin` trait gains `Send + Sync`

```rust
// core/plugin.rs:52
pub trait Plugin: Send + Sync {  // ← added
    fn requires(&self, _decls: &mut CapabilityDecls) {}
    fn compile(&self, _cx: &mut CompileContext) -> Result<(), TurError> { Ok(()) }
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError>;
}
```

All six concrete plugins (zero-field unit structs) are trivially `Send + Sync`. Update integration-test plugins (`HostModulePlugin`, `NeedsCounterPlugin`, `CapturePlugin`, `CounterPlugin`) — verify they're `Send + Sync` (likely yes if they don't hold `Rc`/`RefCell`).

`TurRuntime::plugins: Vec<Box<dyn Plugin>>` automatically becomes `Send + Sync` via the trait bound.

### 6.4 — `NativeHttp::new()` drops Handle param

Today (`tur-net-native/src/backend.rs:58-63`):
```rust
impl NativeHttp {
    pub fn new(handle: tokio::runtime::Handle) -> Self { ... }
}
```

Tomorrow:
```rust
impl NativeHttp {
    pub fn new() -> Self { NativeHttp }
}

impl HttpBackend for NativeHttp {
    fn request(&self, req: Request) -> BoxFuture<...> {
        // Pull the Handle from the engine's runtime context (set by
        // TurRuntimeBuilder::tokio_handle in Phase 7). For Phase 6
        // (still single-threaded), use a thread-local or context param.
        let handle = current_tokio_handle();
        handle.spawn(...);
        ...
    }
}
```

Phase 6 transitional approach: a `thread_local!` or `Rc<RefCell<Option<Handle>>>` set by the runtime at construction. Phase 7 cleans this up to a proper runtime-context injection.

Android auto-installs `TurNetPlugin` (no Handle plumbing required).

### 6.5 — `tur-net-native` stream Send-ness

Verify `MpscBodyStream` (`tur-net-native/src/backend.rs:151-192`) is `Send`. The earlier audit confirmed `StreamMsg` is `Send`. The stream's `poll_next` calls `self.0.poll_recv(cx)` (reactor-agnostic). Should be Send as-is. Confirm via `Send` assertion.

### 6.6 — Compile-time assertions

Add to `core/capability.rs`:
```rust
const _: fn = || {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Clipboard>();
    assert_sync::<Clipboard>();
    assert_send::<Http>();
    assert_sync::<Http>();
    assert_send::<FilePicker>();
    assert_sync::<FilePicker>();
};
```

Same for backends and Plugin.

## Files to modify

- `libs/tur-engine/src/core/plugin.rs` — `Plugin: Send + Sync`
- `libs/tur-engine/src/core/capability.rs` — Send/Sync assertions
- `libs/tur-engine/src/builtin_plugins/clipboard/capability.rs` — `Clipboard(Arc<... + Send + Sync>)`
- `libs/tur-engine/src/builtin_plugins/clipboard/mod.rs` — update `requires` and builder plumbing
- `libs/tur-net-capability/src/lib.rs` — verify `HttpBackend: Send + Sync`; ensure `Http` newtype is Send + Sync
- `libs/tur-filepicker-capability/src/lib.rs` — same
- `libs/tur-net-native/src/backend.rs` — `NativeHttp::new()` no Handle; backend reads from context
- `libs/tur-net-wasm/src/backend.rs` — verify Send
- `libs/tur-clipboard-{wasm,native,android}/src/lib.rs` — verify Send + Sync
- `libs/tur-filepicker-{wasm,native}/src/lib.rs` — verify
- `libs/tur-engine/src/core/runtime.rs` — `Capabilities` registry uses `Arc<dyn Any + Send + Sync>` instead of `Rc<dyn Any>`
- `libs/tur-android/src/app.rs` — auto-install `TurNetPlugin`

## Verification

1. Compile-time `Send`/`Sync` assertions in `capability.rs` fire if any backend breaks the contract.
2. `cargo test --workspace` — all tests pass (single-threaded behavior unchanged).
3. `cargo build --workspace` clean.
4. `cargo clippy --workspace -- -D warnings`.
5. `cargo check --target wasm32-unknown-unknown` (wasm backends must also be Send + Sync, even though wasm is single-threaded today — future-proofs for Phase 9).

## Risks

- **`arboard` thread affinity on macOS** — earlier audit flagged `arboard` historically requires main thread on macOS for `NSPasteboard`. Verify on each platform; if macOS needs main-thread, the clipboard backend must proxy via a main-thread channel (one extra RPC, rare op).
- **`TurRuntime::plugins: Vec<Box<dyn Plugin + Send + Sync>>`** — changes the storage type; minor ripple through runtime builder code.
- **`Capabilities` registry `Rc` → `Arc`** — slightly more overhead per capability lookup (atomic vs non-atomic). Negligible.

## Out of scope

- Multi-threading — still single-threaded.
- Worker pool — Phase 7.
- tokio runtime injection (full version) — Phase 7.

## Estimated scope

- ~150 lines modified across many small files (mostly signature changes)
- ~50 lines new (assertions, docs)
- Mechanical PR; mostly review by grep
