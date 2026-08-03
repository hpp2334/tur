//! Platform-abstracted thread primitives.
//!
//! Re-exports `wasm_thread` on `wasm32-unknown-unknown` (Web Workers backed
//! by `SharedArrayBuffer`) and `std::thread` elsewhere. Both expose the
//! same `Builder` / `JoinHandle` / `spawn` / `Thread` / `ThreadId` / `sleep`
//! API surface, so callers can write target-agnostic thread-spawning code.
//!
//! Wasm builds require the atomics + shared-memory + build-std config in
//! `.cargo/config.toml` + `--profile wasm-dev`.

#[cfg(target_arch = "wasm32")]
pub use wasm_thread::{Builder, JoinHandle, Thread, ThreadId, sleep, spawn};

#[cfg(not(target_arch = "wasm32"))]
pub use std::thread::{Builder, JoinHandle, Thread, ThreadId, sleep, spawn};
