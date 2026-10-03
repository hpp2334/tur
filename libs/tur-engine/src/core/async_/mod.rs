//! Engine-side async plumbing.
//!
//! The script rail (rut) drives its own async weave: capability rows spawn
//! worker futures via [`InstanceContext::spawn_local`] and settle
//! [`rut_vm::Completer`]s; the pump's `run_ready` drives the awaiting tasks.
//!
//! The engine contributes the worker-side task context:
//! [`AsyncWorkerContext`] — timers / nested spawns / blocking work / the
//! self-waking paint signal (e.g. the caret-blink loop).

pub mod async_worker_context;

pub use async_worker_context::AsyncWorkerContext;
