//! Worker-side virtual-app state — shared between the rut rows, the
//! `VirtualAppSubsystem`, and the `VirtualAppView` element (all on the
//! parent's worker). One `Rc<VirtualState>` per instance, created in
//! `install_virtual_app`.
//!
//! Identity model:
//! - A **controller** (`va_controller`) has a stable `base` id — that's
//!   what the author-side opaque carries and what records are keyed by.
//! - Each **spawn** allocates a fresh incarnation `token`
//!   ([`VirtualAppId`]) — so a rapid destroy/re-bind can never race two
//!   children under one identity (host + outputs are keyed by token).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::core::app::HostTx;
use crate::core::app::comm::HostMsg;
use crate::core::app::runtime_error::RuntimeErrorReport;
use crate::core::edgy::mutation::{MutationHandle, MutationPayload, PendingMutationInvocationQueue};
use crate::core::edgy::reactive::{ReactiveBridgeStore, Source};
use crate::core::edgy::value::Value;
use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::RenderCommandBatch;
use crate::core::scheduler::WorkerPoolHandle;
use crate::core::virtual_app::{VirtualAppId, VirtualControl};

/// Stable per-controller identity carried by the author-side opaque (the
/// rut rows wrap the bare `u64`; this newtype is the typed view the
/// element's `app$` val decodes to).
#[derive(Debug, Clone)]
pub(crate) struct VirtualControllerRef(pub(crate) u64);

impl crate::core::edgy::FromValue for VirtualControllerRef {
    /// A reactive `app$` val carries the ref opaquely (`Value::Opaque`
    /// holding an `Rc<VirtualControllerRef>` — identity-preserving, the
    /// same escape hatch every engine handle rides).
    fn from_value(value: &Value) -> Result<Self, String> {
        value
            .as_opaque()
            .and_then(|o| o.clone().downcast::<VirtualControllerRef>().ok())
            .map(|rc| (*rc).clone())
            .ok_or_else(|| crate::core::edgy::value::type_error("a virtual app controller"))
    }
}

/// The `onRuntimeError$` callback argument. The child's thrown value never
/// crosses the worker boundary — only its formatted message does.
pub(crate) struct RuntimeErrorArg {
    message: String,
}

impl From<RuntimeErrorReport> for RuntimeErrorArg {
    fn from(report: RuntimeErrorReport) -> Self {
        Self {
            message: report.message,
        }
    }
}

impl MutationPayload for RuntimeErrorArg {
    /// `[message]` — the child's formatted runtime-error message.
    fn to_value_args(&self) -> Vec<Value> {
        vec![Value::str(self.message.as_str())]
    }
}

/// One controller's worker-side record.
pub(crate) struct ControllerRecord {
    pub status: Source<crate::core::edgy::Value>,
    pub error_msg: Source<crate::core::edgy::Value>,
    pub keep_alive: bool,
    /// Resolved target pool (a `forWorkerPool` handle or the default
    /// `"virtual"` pool, resolved at controller creation).
    pub pool: WorkerPoolHandle,
    pub source: Arc<str>,
    /// The `onRuntimeError$` mutation (optional) — dispatched when the
    /// hosted child reports a runtime JS error.
    pub on_runtime_error: Option<MutationHandle<RuntimeErrorArg>>,
    /// Last flush epoch an error was dispatched in — the per-frame
    /// coalescing guard (a throwing-every-frame child cannot flood the
    /// mutation queue). `u64::MAX` = never.
    pub error_dispatch_frame: Cell<u64>,
    /// Live incarnation token, if this controller currently hosts a child
    /// (cleared by `destroy$` / unbind-destroy; a later bind respawns under
    /// a fresh token).
    pub current: Cell<Option<VirtualAppId>>,
    /// The binder instance that owns the live incarnation (see
    /// [`VirtualState::bind`]). `None` while no incarnation is live.
    pub binder: Cell<Option<u64>>,
    /// An element currently binds this controller (gates the subsystem's
    /// post-layout rect walk).
    pub bound: Cell<bool>,
    /// Last rect shipped via `VirtualControl::Resize` (dedup) —
    /// `(x, y, width, height)`.
    pub last_rect: Cell<(f64, f64, f64, f64)>,
}

/// A child's latest paint, as replayed by the host element.
pub(crate) struct ChildOutput {
    pub batch: Arc<RenderCommandBatch>,
    /// Child image ids re-keyed into the parent's `ImageResourceId` space
    /// (child ids are per-instance and would collide).
    pub image_remap: HashMap<ImageResourceId, ImageResourceId>,
}

/// The shared per-instance virtual-app state (register-phase plugin state,
/// read back through `InstanceContext::plugin_state::<VirtualState>()`).
/// Public so embedder-side services (the playground's `pg_compile` row) can
/// mint sources on the Rust side ([`VirtualState::create_source`]) — the
/// rut realm then lifts the bare `u64` through `va_source_handle`.
pub struct VirtualState {
    pub(crate) host_tx: HostTx,
    pub(crate) bridge: ReactiveBridgeStore,
    next_id: Cell<u64>,
    sources: RefCell<HashMap<u64, Arc<str>>>,
    pub(crate) controllers: RefCell<HashMap<u64, Rc<ControllerRecord>>>,
    /// incarnation token → controller base (for status-event routing).
    tokens: RefCell<HashMap<u64, u64>>,
    pub(crate) outputs: RefCell<HashMap<u64, ChildOutput>>,
}

impl VirtualState {
    pub(crate) fn new(host_tx: HostTx, bridge: ReactiveBridgeStore) -> Self {
        Self {
            host_tx,
            bridge,
            next_id: Cell::new(1),
            sources: RefCell::new(HashMap::new()),
            controllers: RefCell::new(HashMap::new()),
            tokens: RefCell::new(HashMap::new()),
            outputs: RefCell::new(HashMap::new()),
        }
    }

    fn alloc_id(&self) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }

    // ── module sources ────────────────────────────────────────────────

    pub fn create_source(&self, source: Arc<str>) -> u64 {
        let id = self.alloc_id();
        self.sources.borrow_mut().insert(id, source);
        id
    }

    pub(crate) fn resolve_source(&self, handle: u64) -> Option<Arc<str>> {
        self.sources.borrow().get(&handle).cloned()
    }

    // ── controllers ───────────────────────────────────────────────────

    pub(crate) fn create_controller(
        &self,
        source: Arc<str>,
        pool: WorkerPoolHandle,
        keep_alive: bool,
        on_runtime_error: Option<MutationHandle<RuntimeErrorArg>>,
    ) -> u64 {
        let base = self.alloc_id();
        let status = self
            .bridge
            .decl_source(crate::core::edgy::Value::str("idle"));
        let error_msg = self.bridge.decl_source(crate::core::edgy::Value::str(""));
        self.controllers.borrow_mut().insert(
            base,
            Rc::new(ControllerRecord {
                status,
                error_msg,
                keep_alive,
                pool,
                source,
                on_runtime_error,
                error_dispatch_frame: Cell::new(u64::MAX),
                current: Cell::new(None),
                binder: Cell::new(None),
                bound: Cell::new(false),
                last_rect: Cell::new((-1.0, -1.0, -1.0, -1.0)),
            }),
        );
        base
    }

    pub(crate) fn record(&self, base: u64) -> Option<Rc<ControllerRecord>> {
        self.controllers.borrow().get(&base).cloned()
    }

    /// The controller record owning a live incarnation `token`.
    pub(crate) fn record_by_token(&self, token: VirtualAppId) -> Option<Rc<ControllerRecord>> {
        let base = self.tokens.borrow().get(&token.0).copied()?;
        self.record(base)
    }

    /// The live child token whose host element is `binder` — the key
    /// forwarder's resolution of "which child holds focus" from the
    /// parent's focus manager (keys are focus-routed, not hit-tested).
    /// `None` when no bound controller's host matches.
    pub(crate) fn focused_child_token(
        &self,
        binder: crate::core::element::ElementNodeId,
    ) -> Option<VirtualAppId> {
        let binder = u64::from(binder);
        let controllers = self.controllers.borrow();
        for record in controllers.values() {
            if record.binder.get() == Some(binder)
                && record.bound.get()
                && let Some(token) = record.current.get()
            {
                return Some(token);
            }
        }
        None
    }

    // ── bind / unbind (driven by the element's layout diff) ───────────

    /// An element binds the controller: spawn a child if none is live.
    ///
    /// `binder` is the binding element's instance id. The live incarnation
    /// is OWNED by its binder: a bind from the same instance is a keep-alive
    /// rebind (no-op), while a bind from a DIFFERENT instance is a takeover
    /// — the previous holder is being torn down in this same flush (the
    /// Switch's mount-new-before-destroy-old order), so its incarnation is
    /// retired here and a fresh one spawns for the new holder. That holder's
    /// own `unbind` then arrives stale and no-ops.
    pub(crate) fn bind(&self, base: u64, binder: u64) {
        let Some(record) = self.record(base) else {
            return;
        };
        record.bound.set(true);
        if record.current.get().is_some() {
            if record.binder.get() == Some(binder) {
                return; // same element re-binding its live incarnation
            }
            // Takeover: retire the previous holder's incarnation.
            self.retire(base, &record);
        }
        record.binder.set(Some(binder));
        let token = VirtualAppId(self.alloc_id());
        record.current.set(Some(token));
        self.tokens.borrow_mut().insert(token.0, base);
        self.set_status(base, "spawning", "");
        self.send_control(VirtualControl::Spawn {
            token,
            source: record.source.clone(),
            pool: record.pool.clone(),
        });
    }

    /// The element stops binding the controller: destroy the child unless
    /// `keepAlive`. A stale unbind — from a former holder whose incarnation
    /// was already taken over (see [`Self::bind`]) — is a no-op (it must not
    /// clear `bound` either: the new holder's input-forwarding rect walk
    /// gates on it).
    pub(crate) fn unbind(&self, base: u64, binder: u64) {
        let Some(record) = self.record(base) else {
            return;
        };
        if record.binder.get() != Some(binder) {
            return; // the incarnation belongs to another binder now
        }
        record.bound.set(false);
        if !record.keep_alive {
            self.retire(base, &record);
        }
    }

    /// Explicit destroy (the rut rail's `va_destroy` row) — always retires,
    /// regardless of `keepAlive`.
    pub(crate) fn destroy(&self, base: u64) {
        if let Some(record) = self.record(base) {
            record.bound.set(false);
            self.retire(base, &record);
        }
    }

    fn retire(&self, base: u64, record: &ControllerRecord) {
        record.binder.set(None);
        if let Some(token) = record.current.take() {
            self.set_status(base, "destroyed", "");
            self.send_control(VirtualControl::Destroy { token });
            // Outputs are cleared when the host confirms (`Destroyed`
            // status event) — the child may ship one last frame before it
            // tears down.
        }
    }

    // ── status / events ───────────────────────────────────────────────

    pub(crate) fn set_status(&self, base: u64, status: &str, error: &str) {
        let Some(record) = self.record(base) else {
            return;
        };
        let _ = self
            .bridge
            .set_source(record.status, crate::core::edgy::Value::str(status));
        let _ = self
            .bridge
            .set_source(record.error_msg, crate::core::edgy::Value::str(error));
    }

    /// Route a `Destroyed` confirmation for an incarnation token.
    pub(crate) fn handle_destroyed(&self, token: VirtualAppId) {
        self.outputs.borrow_mut().remove(&token.0);
        if let Some(base) = self.tokens.borrow_mut().remove(&token.0)
            && let Some(record) = self.record(base)
        {
            // Only flip status if this was still the live incarnation
            // (a respawn may already be running under a newer token).
            if record.current.get().is_none_or(|current| current == token) {
                if record.current.get() == Some(token) {
                    record.current.set(None);
                }
                self.set_status(base, "destroyed", "");
            }
        }
    }

    pub(crate) fn handle_status(
        &self,
        token: VirtualAppId,
        state: crate::core::virtual_app::VirtualStatusState,
        detail: Option<&str>,
    ) {
        use crate::core::virtual_app::VirtualStatusState::*;
        let Some(base) = self.tokens.borrow().get(&token.0).copied() else {
            return;
        };
        match state {
            Running => {
                self.set_status(base, "running", "");
                // Re-ship the rect: the first `Resize` may have raced the
                // spawn (dropped host-side before the child existed).
                if let Some(record) = self.record(base) {
                    record.last_rect.set((-1.0, -1.0, -1.0, -1.0));
                }
            }
            Error => self.set_status(base, "error", detail.unwrap_or("unknown error")),
            Destroyed => self.handle_destroyed(token),
        }
    }

    /// A runtime JS error reported by the hosted child (the
    /// `onRuntimeError$` rail). Dispatches the controller's mutation onto
    /// the mutation queue — the `watch` convention: same-frame drain by the
    /// flush fixed point, serialized ordering. At most one dispatch per
    /// flush epoch per controller. A late event for a retired incarnation
    /// (unknown token) is dropped.
    pub(crate) fn handle_runtime_error(
        &self,
        token: VirtualAppId,
        report: RuntimeErrorReport,
        frame_id: u64,
        queue: &Rc<RefCell<PendingMutationInvocationQueue>>,
    ) {
        let Some(base) = self.tokens.borrow().get(&token.0).copied() else {
            return; // retired incarnation — nothing to dispatch to
        };
        let Some(record) = self.record(base) else {
            return;
        };
        let Some(handle) = record.on_runtime_error else {
            return; // controller has no onRuntimeError$ — status rail only
        };
        if record.error_dispatch_frame.get() == frame_id {
            return; // already dispatched this frame — coalesce
        }
        record.error_dispatch_frame.set(frame_id);
        queue
            .borrow_mut()
            .push(handle, RuntimeErrorArg::from(report));
    }

    // ── controls ──────────────────────────────────────────────────────

    pub(crate) fn send_control(&self, control: VirtualControl) {
        let _ = self
            .host_tx
            .unbounded_send(HostMsg::VirtualControl(control));
    }

    /// Ship a deduped shell command to this instance's host surface — the
    /// child text-input egress path: the parent's worker re-ships the
    /// (caret-translated) request so the embedder's shell raises/positions
    /// the text-input surface for a child's focused editable.
    pub(crate) fn ship_shell(&self, cmd: crate::core::app::comm::ShellCommand) {
        let _ = self.host_tx.unbounded_send(HostMsg::Shell(cmd));
    }

    // ── outputs ───────────────────────────────────────────────────────

    pub(crate) fn store_frame(
        &self,
        token: VirtualAppId,
        batch: Arc<RenderCommandBatch>,
        images: Vec<(ImageResourceId, ImageResource)>,
        register_image: impl Fn(ImageResource) -> ImageResourceId,
    ) {
        let mut remap = HashMap::new();
        for (child_id, image) in images {
            let parent_id = register_image(image);
            remap.insert(child_id, parent_id);
        }
        // Carry over remaps from previous frames — a child image is
        // uploaded once but referenced by every subsequent batch.
        let existing = self
            .outputs
            .borrow()
            .get(&token.0)
            .map(|o| o.image_remap.clone())
            .unwrap_or_default();
        remap.extend(existing);
        self.outputs.borrow_mut().insert(
            token.0,
            ChildOutput {
                batch,
                image_remap: remap,
            },
        );
    }

    pub(crate) fn output(&self, token: VirtualAppId) -> Option<Arc<RenderCommandBatch>> {
        self.outputs.borrow().get(&token.0).map(|o| o.batch.clone())
    }

    pub(crate) fn image_remap(
        &self,
        token: VirtualAppId,
    ) -> HashMap<ImageResourceId, ImageResourceId> {
        self.outputs
            .borrow()
            .get(&token.0)
            .map(|o| o.image_remap.clone())
            .unwrap_or_default()
    }

    /// Whether any controller is currently bound (gates the subsystem's
    /// post-layout rect walk — O(tree) is only paid while hosting).
    pub(crate) fn any_bound(&self) -> bool {
        self.controllers.borrow().values().any(|r| r.bound.get())
    }
}
