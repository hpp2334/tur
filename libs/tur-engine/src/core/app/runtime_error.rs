//! Runtime-error reporting — the worker-side rail for JS errors that no
//! caller can observe: a throwing event handler / view closure / factory /
//! microtask / async callback, or a promise rejection with no handler.
//! The instance's [`RuntimeErrorReporter`] ships each as a
//! [`HostMsg::RuntimeError`] to its host; an element-hosted virtual app's
//! host forwards it to the parent's worker (`VirtualErrorEvent` → the
//! controller's `onRuntimeError$`), while the embedder-hosted root's looper
//! just logs it.
//!
//! The reporter rides on the boa `Context` as host-defined data
//! (`insert_data` / `get_data`), so every capture site — each of which
//! already holds `&mut Context` — reaches it through the [`report`]
//! helper with no signature changes.
//!
//! Promise rejections are tracked by us: the job executor and the host
//! hooks are ours, so no external tracker is needed.
//! [`PromiseRejectionHandler`] installs the standard ECMA-262
//! `HostHooks::promise_rejection_tracker`: `Reject` parks the promise in a
//! pending set, `Handle` retracts it, and
//! [`RuntimeErrorReporter::report_pending_rejections`] — called at the end
//! of each `flush()` — reports whatever still has no handler. A `.catch` /
//! `await` attached within the same turn therefore retracts (browser-like
//! semantics), and there is no double-reporting: the tracker fires `Reject`
//! only for promises with no handler, while a handled rejection whose
//! handler *throws* surfaces through the ordinary job-error capture.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::builtins::promise::{OperationType, Promise, PromiseState};
use boa_engine::context::HostHooks;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsPromise;
use boa_engine::{Context, JsError, JsValue, js_string};

use crate::core::app::comm::{HostMsg, HostTx};

/// A JS runtime error crossing the worker → host boundary. The thrown value
/// itself never crosses (the realms differ); its `message` and best-effort
/// `stack` do.
#[derive(Debug, Clone)]
pub struct RuntimeErrorReport {
    pub message: String,
    pub stack: Option<String>,
}

impl RuntimeErrorReport {
    /// Format a thrown [`JsError`] — `message` (the thrown object's own
    /// `.message` when present, e.g. `"boom"` for `new Error("boom")`)
    /// plus a best-effort `.stack`.
    pub fn of(err: &JsError, boa: &mut Context) -> Self {
        match err.clone().into_opaque(boa) {
            Ok(value) => Self::of_value(&value, boa),
            // Opaque conversion failing is a degenerate case (the docs show
            // it only for exotic payloads); the display form still carries
            // the essentials.
            Err(_) => Self {
                message: err.to_string(),
                stack: None,
            },
        }
    }

    /// Format a raw thrown value (the promise-rejection path — the reason
    /// is a plain [`JsValue`], not a [`JsError`]).
    fn of_value(value: &JsValue, boa: &mut Context) -> Self {
        let message = value
            .as_object()
            .and_then(|o| o.get(js_string!("message"), boa).ok())
            .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
            .unwrap_or_else(|| value.display().to_string());
        let stack = value
            .as_object()
            .and_then(|o| o.get(js_string!("stack"), boa).ok())
            .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()));
        Self { message, stack }
    }
}

struct Inner {
    host_tx: HostTx,
    /// Rejections parked by [`PromiseRejectionHandler`] (identity-compared;
    /// rejections are rare). Drained by
    /// [`RuntimeErrorReporter::report_pending_rejections`] at the end of
    /// each flush.
    pending_rejections: RefCell<Vec<JsObject<Promise>>>,
}

/// Per-instance runtime-error reporter. A cheap-clone `Rc` wrapper — one
/// identity per instance, built in `build_worker_backend` and installed on
/// the boa `Context` as host-defined data. Sends are fire-and-forget (the
/// `HostMsg::UploadImage` pattern).
#[derive(Clone)]
pub struct RuntimeErrorReporter(Rc<Inner>);

impl RuntimeErrorReporter {
    pub fn new(host_tx: HostTx) -> Self {
        Self(Rc::new(Inner {
            host_tx,
            pending_rejections: RefCell::new(Vec::new()),
        }))
    }

    /// Report a thrown JS error to the instance's host.
    pub fn report_js_error(&self, err: &JsError, boa: &mut Context) {
        self.send(RuntimeErrorReport::of(err, boa));
    }

    /// `HostPromiseRejectionTracker(promise, "reject")` — the promise was
    /// rejected with no handler attached.
    pub fn on_rejected(&self, promise: &JsObject<Promise>) {
        self.0.pending_rejections.borrow_mut().push(promise.clone());
    }

    /// `HostPromiseRejectionTracker(promise, "handle")` — a handler was
    /// attached to the rejected promise: retract it.
    pub fn on_handled(&self, promise: &JsObject<Promise>) {
        self.0
            .pending_rejections
            .borrow_mut()
            .retain(|p| !JsObject::equals(p, promise));
    }

    /// End-of-turn reporting: rejections that still have no handler. Called
    /// at the end of `flush()` — the turn boundary within which a
    /// `.catch` / `await` retracts.
    pub fn report_pending_rejections(&self, boa: &mut Context) {
        let pending = std::mem::take(&mut *self.0.pending_rejections.borrow_mut());
        for promise in pending {
            // A parked promise is always rejected at this point (rejection
            // is what parked it, and state is settled forever after); skip
            // defensively anyway.
            let reason = match JsPromise::from(promise).state() {
                PromiseState::Rejected(reason) => reason,
                _ => continue,
            };
            let report = RuntimeErrorReport::of_value(&reason, boa);
            self.send(report);
        }
    }

    fn send(&self, report: RuntimeErrorReport) {
        let _ = self
            .0
            .host_tx
            .unbounded_send(HostMsg::RuntimeError { report });
    }
}

/// Report a thrown JS error through the instance's reporter (read off the
/// boa `Context`). No-op when no reporter is installed (bare test realms).
pub fn report(ctx: &mut Context, err: &JsError) {
    if let Some(reporter) = ctx.get_data::<RuntimeErrorReporter>().cloned() {
        reporter.report_js_error(err, ctx);
    }
}

/// [`HostHooks`] override installing our promise-rejection tracking. Only
/// `promise_rejection_tracker` is overridden; every other hook keeps its
/// trait default.
pub struct PromiseRejectionHandler {
    reporter: RuntimeErrorReporter,
}

impl PromiseRejectionHandler {
    pub fn new(reporter: RuntimeErrorReporter) -> Self {
        Self { reporter }
    }
}

impl HostHooks for PromiseRejectionHandler {
    fn promise_rejection_tracker(
        &self,
        promise: &JsObject<Promise>,
        operation: OperationType,
        _context: &mut Context,
    ) {
        match operation {
            OperationType::Reject => self.reporter.on_rejected(promise),
            OperationType::Handle => self.reporter.on_handled(promise),
        }
    }
}
