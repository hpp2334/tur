//! Runtime-error reporting — the worker-side rail for module errors that no
//! caller can observe: a trapping event entry / view closure / derive /
//! async task, or fuel exhaustion during a face call. Each report ships as
//! a [`HostMsg::RuntimeError`] to the instance's host; an element-hosted
//! virtual app's host forwards it to the parent's worker
//! (`VirtualErrorEvent` → the controller's `onRuntimeError$`), while the
//! embedder-hosted root's looper just logs it.
//!
//! Producers are the rut rail's guard rails (see `core::rut_runtime`):
//! face-call traps, task traps, and fuel exhaustion.

use crate::core::app::comm::HostMsg;
use crate::core::app::comm::HostTx;

/// A runtime error crossing the worker → host boundary. The module's own
/// error payload never crosses verbatim — its formatted `message` and a
/// best-effort `stack` do.
#[derive(Debug, Clone)]
pub struct RuntimeErrorReport {
    pub message: String,
    pub stack: Option<String>,
}

/// Report a formatted runtime error to the instance's host
/// (fire-and-forget — the `HostMsg::UploadImage` pattern).
pub fn send_report(host_tx: &HostTx, message: String, stack: Option<String>) {
    let _ = host_tx.unbounded_send(HostMsg::RuntimeError {
        report: RuntimeErrorReport { message, stack },
    });
}
