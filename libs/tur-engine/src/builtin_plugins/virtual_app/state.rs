//! Worker-side virtual-app state — shared between the bridge fns, the
//! `VirtualAppSubsystem`, and the `VirtualAppView` element (all on the
//! parent's worker). One `Rc<VirtualState>` per instance, created in
//! `install_virtual_app`.
//!
//! Identity model:
//! - A **controller** (`createVirtualAppController`) has a stable `base`
//!   id — that's what the JS handle carries and what records are keyed by.
//! - Each **spawn** allocates a fresh incarnation `token`
//!   ([`VirtualAppId`]) — so a rapid destroy/re-bind can never race two
//!   children under one identity (host + outputs are keyed by token).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use boa_engine::{Context, JsValue, js_string};
use boa_gc::{Finalize, Trace};

use crate::core::app::HostTx;
use crate::core::app::comm::HostMsg;
use crate::core::edgy::reactive::{Mutation, ReactiveBridgeStore, Source};
use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::js_runtime::js_value::IntoJs;
use crate::core::render::RenderCommandBatch;
use crate::core::scheduler::WorkerPoolHandle;
use crate::core::virtual_app::{VirtualAppId, VirtualControl};

/// JS-opaque handle returned by `createModuleSource` — the source string
/// never crosses the JS API again, only this id does.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub(crate) struct ModuleSourceHandle(pub(crate) u64);

impl crate::core::js_runtime::js_value::IntoJs for ModuleSourceHandle {
    fn into_js(self, ctx: &mut Context) -> JsValue {
        let proto = ctx.intrinsics().constructors().object().prototype();
        boa_engine::JsObject::from_proto_and_data(proto, self).into()
    }
}

/// JS-opaque handle returned by `forWorkerPool(name)` — wraps the very
/// `WorkerPoolHandle` the embedder registered (resolved eagerly against the
/// runtime's registry), so a controller built with it spawns its child
/// into exactly the pool Rust code would have assigned. Unforgeable from
/// JS: the payload can only be minted by the bridge.
#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub(crate) struct JsWorkerPoolHandle(pub(crate) WorkerPoolHandle);

impl crate::core::js_runtime::js_value::IntoJs for JsWorkerPoolHandle {
    fn into_js(self, ctx: &mut Context) -> JsValue {
        let proto = ctx.intrinsics().constructors().object().prototype();
        boa_engine::JsObject::from_proto_and_data(proto, self).into()
    }
}

/// Stable per-controller identity carried by the JS controller object.
#[derive(Debug, Clone)]
pub(crate) struct VirtualControllerRef(pub(crate) u64);

impl crate::core::js_runtime::js_value::FromJs for VirtualControllerRef {
    fn from_js(value: &JsValue) -> Result<Self, boa_engine::JsError> {
        let obj = value.as_object().ok_or_else(|| {
            crate::core::js_runtime::js_value::type_error("a virtual app controller")
        })?;
        obj.downcast_ref::<JsVirtualController>()
            .map(|c| VirtualControllerRef(c.0))
            .ok_or_else(|| {
                crate::core::js_runtime::js_value::type_error("a virtual app controller")
            })
    }
}

/// `JsData` payload of the JS controller object. The object also carries
/// `status$` / `errorMsg$` / `destroy$` properties.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub(crate) struct JsVirtualController(pub(crate) u64);

/// One controller's worker-side record.
pub(crate) struct ControllerRecord {
    pub status: Source<JsValue>,
    pub error_msg: Source<JsValue>,
    pub keep_alive: bool,
    /// Resolved target pool (a `forWorkerPool` handle or the default
    /// `"virtual"` pool, resolved at controller creation).
    pub pool: WorkerPoolHandle,
    pub source: Arc<str>,
    /// Live incarnation token, if this controller currently hosts a child
    /// (cleared by `destroy$` / unbind-destroy; a later bind respawns under
    /// a fresh token).
    pub current: Cell<Option<VirtualAppId>>,
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

pub(crate) struct VirtualState {
    pub host_tx: HostTx,
    pub bridge: ReactiveBridgeStore,
    next_id: Cell<u64>,
    sources: RefCell<HashMap<u64, Arc<str>>>,
    pub controllers: RefCell<HashMap<u64, Rc<ControllerRecord>>>,
    /// incarnation token → controller base (for status-event routing).
    tokens: RefCell<HashMap<u64, u64>>,
    pub outputs: RefCell<HashMap<u64, ChildOutput>>,
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

    pub(crate) fn register_source(&self, source: Arc<str>) -> u64 {
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
    ) -> u64 {
        let base = self.alloc_id();
        let status = self.bridge.decl_source(JsValue::from(js_string!("idle")));
        let error_msg = self.bridge.decl_source(JsValue::from(js_string!("")));
        self.controllers.borrow_mut().insert(
            base,
            Rc::new(ControllerRecord {
                status,
                error_msg,
                keep_alive,
                pool,
                source,
                current: Cell::new(None),
                bound: Cell::new(false),
                last_rect: Cell::new((-1.0, -1.0, -1.0, -1.0)),
            }),
        );
        base
    }

    pub(crate) fn record(&self, base: u64) -> Option<Rc<ControllerRecord>> {
        self.controllers.borrow().get(&base).cloned()
    }

    /// Build the JS controller object: `JsData(JsVirtualController(base))`
    /// carrying `status$` / `errorMsg$` / `destroy$`.
    pub(crate) fn controller_js_object(
        &self,
        base: u64,
        destroy: Mutation,
        ctx: &mut Context,
    ) -> JsValue {
        let record = self
            .record(base)
            .expect("controller record exists until Destroyed is confirmed");
        let proto = ctx.intrinsics().constructors().object().prototype();
        let obj = boa_engine::JsObject::from_proto_and_data(proto, JsVirtualController(base));
        let _ = obj.create_data_property(js_string!("status$"), record.status.into_js(ctx), ctx);
        let _ =
            obj.create_data_property(js_string!("errorMsg$"), record.error_msg.into_js(ctx), ctx);
        let _ = obj.create_data_property(js_string!("destroy$"), destroy.into_js(ctx), ctx);
        obj.into()
    }

    // ── bind / unbind (driven by the element's layout diff) ───────────

    /// An element binds the controller: spawn a child if none is live.
    pub(crate) fn bind(&self, base: u64) {
        let Some(record) = self.record(base) else {
            return;
        };
        record.bound.set(true);
        if record.current.get().is_some() {
            return; // already hosting (keep-alive rebind)
        }
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
    /// `keepAlive`.
    pub(crate) fn unbind(&self, base: u64) {
        let Some(record) = self.record(base) else {
            return;
        };
        record.bound.set(false);
        if !record.keep_alive {
            self.retire(base, &record);
        }
    }

    /// Explicit destroy (the `destroy$` control mutation) — always retires,
    /// regardless of `keepAlive`.
    pub(crate) fn destroy(&self, base: u64) {
        if let Some(record) = self.record(base) {
            record.bound.set(false);
            self.retire(base, &record);
        }
    }

    fn retire(&self, base: u64, record: &ControllerRecord) {
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
            .set_source(record.status, JsValue::from(js_string!(status)));
        let _ = self
            .bridge
            .set_source(record.error_msg, JsValue::from(js_string!(error)));
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

    // ── controls ──────────────────────────────────────────────────────

    pub(crate) fn send_control(&self, control: VirtualControl) {
        let _ = self
            .host_tx
            .unbounded_send(HostMsg::VirtualControl(control));
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
