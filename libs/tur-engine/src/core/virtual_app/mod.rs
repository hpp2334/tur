//! Virtual apps — the engine seam for hosting nested app instances.
//!
//! A **virtual app** is any tur instance; what differs is only *who hosts
//! it*. The embedder hosts the root instance (canvas/DOM/JNI as its host
//! surface, via the Rust builder); a `VirtualAppView` element hosts a child
//! instance (the element as its host surface, via the JS API). Hosting always
//! means the same four things through the same interfaces — input
//! (translated platform events), viewport (rect-driven resize), frame clock
//! (vsync), and egress handling (`Shell`).
//!
//! This module is the **host-thread half** of that seam (the plugin half —
//! the element, bridge fns, and subsystem — lives in
//! [`builtin_plugins::virtual_app`]):
//!
//! - [`VirtualHost`] — the **instance's host-side core**: identity
//!   ([`VirtualAppId`]), backend rails (it wraps the instance's
//!   [`HostBackend`](crate::core::runtime::HostBackend) — worker sender +
//!   cross-thread wake; frames + status flow back into the parent's
//!   worker through them), frame clock, lifecycle flag, and the children
//!   it hosts (a recursive map). [`TurApp`](crate::TurApp) is the thin
//!   public facade over it; [`TurAppLooper`](crate::TurAppLooper) routes
//!   [`VirtualControl`] messages arriving from the instance's worker
//!   straight to the core — the backend itself knows nothing about
//!   hosting.
//! - [`ForwardingRenderer`] — the child's [`Renderer`]: instead of drawing,
//!   it ships each painted batch (+ pending image uploads) back to the
//!   **parent's worker** as a [`VirtualFrameEvent`] on the existing
//!   `AppEvent::Custom` rail, where the parent's `VirtualAppSubsystem`
//!   stores it for the host element's paint to replay.
//! - [`VirtualShell`] — the child's [`Shell`]: hands the child the PARENT's
//!   vsync source (both loopers wake on the same tick) and (v1) drops
//!   cursor/text-input egress.
//!
//! ## Message flow
//!
//! ```text
//! parent worker ──HostMsg::VirtualControl(Spawn/Resize/PlatformEvent/Destroy)──▶ VirtualHost
//! VirtualHost   ──WorkerMsg::AppEvent(Custom(VirtualStatusEvent | VirtualFrameEvent))──▶ parent worker
//! ```
//!
//! Tokens (`VirtualAppId`) are allocated on the parent's worker (one per
//! controller *incarnation*), so control messages need no reply channel —
//! status flows back asynchronously as custom app events. The instance's
//! own id is assigned at spawn through the internal `spawn_hosted_instance`
//! path (the embedder-facing builder always names
//! [`VirtualAppId::ROOT`]).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::TurRuntime;
use crate::core::app::comm::{WorkerMsg, WorkerTx};
use crate::core::app::{AppEvent, CustomAppEvent, Reply};
use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::platform::PlatformEvent;
use crate::core::render::{RenderCommand, RenderCommandBatch, Renderer};
use crate::core::scheduler::{HostLoop, VsyncSource, WorkerPoolHandle};
use crate::core::shell::Shell;

/// Opaque token identifying one instance. [`VirtualAppId::ROOT`] names the
/// embedder-hosted instance; any other value is a child's
/// parent-minted "incarnation" token (a fresh token per spawn, so a rapid
/// destroy/re-bind can never race two children under one identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualAppId(pub u64);

impl VirtualAppId {
    /// The embedder-hosted root instance. Reserved structurally, not by
    /// convention: the worker-side token counter starts at 1, so `0` can
    /// never name a child — and the only id writers in existence are that
    /// counter and the public build path (which hardcodes `ROOT`).
    pub const ROOT: VirtualAppId = VirtualAppId(0);
}

/// Parent-worker → host control messages, carried by
/// [`HostMsg::VirtualControl`] (the only new `HostMsg` variant; there is no
/// generic custom path in this direction — mirrors `HostMsg::Shell`).
pub enum VirtualControl {
    /// Build a child instance in `pool` and load `source` into it. The pool
    /// is an already-resolved [`WorkerPoolHandle`] — minted worker-side by
    /// `forWorkerPool(name)` (default `"virtual"`), so it is the very
    /// handle the embedder registered. The module outcome surfaces
    /// asynchronously as a [`VirtualStatusEvent`], not via a reply
    /// (matching the wasm readiness model where real readiness is confirmed
    /// by the first RPC await).
    Spawn {
        token: VirtualAppId,
        source: Arc<str>,
        pool: WorkerPoolHandle,
    },
    /// The host element's on-screen rect changed (deduped by the sender).
    /// Drives the child's `resize` (renderer + `viewportSize$`) and is
    /// retained host-side for egress translation.
    Resize {
        token: VirtualAppId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        dpr: f64,
    },
    /// A platform event translated into the child's viewport coordinates
    /// (`position − host element origin`). Raw primitives only — gestures
    /// compose inside the child's own arena.
    PlatformEvent {
        token: VirtualAppId,
        event: PlatformEvent,
    },
    /// Destroy the child (runs its module cleanup in the child worker).
    Destroy { token: VirtualAppId },
}

impl std::fmt::Debug for VirtualControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn { token, source, .. } => f
                .debug_struct("Spawn")
                .field("token", token)
                .field("source_len", &source.len())
                .finish(),
            Self::Resize {
                token,
                x,
                y,
                width,
                height,
                dpr,
            } => f
                .debug_struct("Resize")
                .field("token", token)
                .field("rect", &(x, y, width, height))
                .field("dpr", dpr)
                .finish(),
            Self::PlatformEvent { token, .. } => f
                .debug_struct("PlatformEvent")
                .field("token", token)
                .finish_non_exhaustive(),
            Self::Destroy { token } => f.debug_struct("Destroy").field("token", token).finish(),
        }
    }
}

/// Lifecycle state of a child, mirrored into the parent's reactive store so
/// the parent UI can derive from it (`status$` / `errorMsg$`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualStatusState {
    /// The module loaded and its `start()` ran.
    Running,
    /// The module failed to load / start (see the event's `detail`).
    Error,
    /// The child was destroyed (or never existed — always confirmed).
    Destroyed,
}

/// Status egress: host → parent worker, riding `AppEvent::Custom` (the
/// clipboard-paste pattern — no new `WorkerMsg` variants).
#[derive(Debug)]
pub struct VirtualStatusEvent {
    pub token: VirtualAppId,
    pub state: VirtualStatusState,
    pub detail: Option<String>,
}

impl CustomAppEvent for VirtualStatusEvent {
    fn name(&self) -> &'static str {
        "virtual:status"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// One painted child frame + any image resources decoded since the last
/// frame, shipped host → parent worker via `AppEvent::Custom`. The parent's
/// `VirtualAppSubsystem` stores the batch (latest-wins) and re-keys the
/// image ids into the parent's id space (child ids are per-instance and
/// would collide).
pub struct VirtualFrameEvent {
    pub token: VirtualAppId,
    pub batch: Arc<RenderCommandBatch>,
    pub images: Vec<(ImageResourceId, ImageResource)>,
}

impl std::fmt::Debug for VirtualFrameEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualFrameEvent")
            .field("token", &self.token)
            .field("batch_len", &self.batch.len())
            .field("images", &self.images.len())
            .finish()
    }
}

impl CustomAppEvent for VirtualFrameEvent {
    fn name(&self) -> &'static str {
        "virtual:frame"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Ship a custom app event into the parent's worker and kick it (the queued
/// event is drained by the next `pump()`; the wake re-arms an idle worker).
fn send_app_event(tx: &WorkerTx, wake: &Rc<dyn Fn()>, event: impl CustomAppEvent + 'static) {
    let _ = tx.unbounded_send(WorkerMsg::AppEvent(AppEvent::custom(event)));
    wake();
}

fn send_status(
    tx: &WorkerTx,
    wake: &Rc<dyn Fn()>,
    token: VirtualAppId,
    state: VirtualStatusState,
    detail: Option<String>,
) {
    send_app_event(
        tx,
        wake,
        VirtualStatusEvent {
            token,
            state,
            detail,
        },
    );
}

// ---------------------------------------------------------------------------
// VirtualHost — the instance's host-side core
// ---------------------------------------------------------------------------

/// The host-side core of one tur instance — everything an instance is on
/// the host thread, independent of *who hosts it*:
///
/// - its **identity** ([`VirtualAppId`] — [`VirtualAppId::ROOT`] when the
///   embedder hosts it, a parent-minted token when a `VirtualAppView`
///   element does),
/// - its rails ([`HostBackend`] — worker sender + renderer + shell; frames
///   and RPC flow through them),
/// - its frame clock (`vsync` — handed to children via [`VirtualShell`],
///   so parent + child loopers wake on the same tick),
/// - its lifecycle (`destroyed`, polled by the looper), and
/// - the **children it hosts** (a recursive map — every instance can host
///   others; "every tur instance is a virtual app" is literally the type).
///
/// [`TurApp`](crate::TurApp) is the thin public facade over this core;
/// [`TurAppLooper`](crate::TurAppLooper) drives it, routing
/// `HostMsg::VirtualControl` straight to [`Self::handle_control`] — the
/// backend itself knows nothing about hosting. Children spawn from the
/// same [`TurRuntime`] (shared plugins / capabilities / fonts / clock)
/// while keeping fully isolated realms, stores, and trees.
pub(crate) struct VirtualHost {
    /// This instance's identity (see the type docs).
    id: VirtualAppId,
    runtime: Rc<TurRuntime>,
    host_loop: Rc<dyn HostLoop>,
    vsync: Rc<dyn VsyncSource>,
    backend: Rc<crate::core::runtime::HostBackend>,
    /// Set by [`Self::destroy`]; polled by the looper's vsync wake-ups.
    destroyed: Rc<Cell<bool>>,
    /// token → child instance. Literal recursion: each value is itself a
    /// host (a child can host its own children — bounded by pool caps).
    children: RefCell<HashMap<VirtualAppId, Rc<VirtualHost>>>,
}

impl VirtualHost {
    pub(crate) fn new(
        id: VirtualAppId,
        runtime: Rc<TurRuntime>,
        host_loop: Rc<dyn HostLoop>,
        vsync: Rc<dyn VsyncSource>,
        backend: Rc<crate::core::runtime::HostBackend>,
    ) -> Self {
        Self {
            id,
            runtime,
            host_loop,
            vsync,
            backend,
            destroyed: Rc::new(Cell::new(false)),
            children: RefCell::new(HashMap::new()),
        }
    }

    /// This instance's identity (see the field docs).
    pub(crate) fn id(&self) -> VirtualAppId {
        self.id
    }

    /// The instance's backend rails (worker sender + renderer + shell).
    pub(crate) fn backend(&self) -> &Rc<crate::core::runtime::HostBackend> {
        &self.backend
    }

    /// The instance's frame clock (the looper re-arms it on
    /// `Vsync`-scheduled outcomes; the facade re-arms it from input paths).
    pub(crate) fn vsync(&self) -> &Rc<dyn VsyncSource> {
        &self.vsync
    }

    /// Polled by the looper's vsync wake-ups (set by [`Self::destroy`]).
    pub(crate) fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    /// The hosted children as host cores. The public surface
    /// (`TurApp::virtual_apps`) mints `TurApp` facades over them — facades
    /// are cheap and minted per call, so identity lives here, on the core.
    pub(crate) fn children(&self) -> Vec<Rc<VirtualHost>> {
        self.children.borrow().values().cloned().collect()
    }

    /// The single control entry point (routed from the looper — the
    /// `host_rx` drain point).
    pub(crate) fn handle_control(&self, control: VirtualControl) {
        match control {
            VirtualControl::Spawn {
                token,
                source,
                pool,
            } => {
                self.spawn_child(token, source, pool);
            }
            VirtualControl::Resize {
                token,
                x: _,
                y: _,
                width,
                height,
                dpr,
            } => {
                if let Some(child) = self.children.borrow().get(&token) {
                    child.resize(width.max(0.0) as u32, height.max(0.0) as u32, dpr);
                }
            }
            VirtualControl::PlatformEvent { token, event } => {
                if let Some(child) = self.children.borrow().get(&token) {
                    child.push_platform_event(event);
                }
            }
            VirtualControl::Destroy { token } => {
                if let Some(child) = self.children.borrow_mut().remove(&token) {
                    child.destroy();
                }
                // Always confirm — covers destroy of a child that never
                // spawned (the worker clears its record + outputs either way).
                send_status(
                    self.backend.worker_tx(),
                    &self.backend.worker_wake_handle(),
                    token,
                    VirtualStatusState::Destroyed,
                    None,
                );
            }
        }
    }

    /// Resize the surface. The single implementation — the facade and the
    /// `Resize` control arm both land here. Resizes the host-side renderer
    /// directly (no flush + worker→host round-trip — lower latency) AND
    /// forwards the shell `Resize` event to the worker so `ResizeSubsystem`
    /// updates `Screen` / `viewportSize$` for layout.
    pub(crate) fn resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.backend.resize(logical_width, logical_height, dpr);
        self.backend
            .send_worker_msg(WorkerMsg::PlatformEvent(PlatformEvent::Shell(
                crate::core::shell::ShellEvent::Resize {
                    logical_width,
                    logical_height,
                    dpr,
                },
            )));
        self.request_frame();
    }

    /// Push a platform (input) event into the instance's worker (the
    /// facade takes `impl Into<PlatformEvent>` and converts first).
    pub(crate) fn push_platform_event(&self, event: PlatformEvent) {
        self.backend
            .send_worker_msg(WorkerMsg::PlatformEvent(event));
        self.request_frame();
    }

    /// Re-arm an idle autonomous loop: ask the vsync source for one
    /// wake-up on the next frame. Idempotent at the source (armed flag).
    pub(crate) fn request_frame(&self) {
        self.vsync.request_frame();
    }

    /// Mark the instance destroyed. Tears down every hosted child FIRST
    /// (engine-owned; each child's module cleanup runs in its own worker),
    /// then sets the flag the looper polls, then sends `WorkerMsg::Destroy`
    /// to drain the worker (fire-and-forget — awaiting would block a sync
    /// API).
    pub(crate) fn destroy(&self) {
        self.destroy_children();
        self.destroyed.set(true);
        let (tx, _rx) = Reply::<()>::pair();
        self.backend
            .send_worker_msg(WorkerMsg::Destroy { reply: tx });
    }

    /// Destroy every hosted child (the first half of [`Self::destroy`]).
    fn destroy_children(&self) {
        for (_, child) in self.children.borrow_mut().drain() {
            child.destroy();
        }
    }

    fn spawn_child(&self, token: VirtualAppId, source: Arc<str>, pool: WorkerPoolHandle) {
        debug_assert!(
            token != VirtualAppId::ROOT,
            "ROOT is reserved for embedder-hosted instances"
        );
        let renderer = ForwardingRenderer::new(
            self.backend.worker_tx().clone(),
            self.backend.worker_wake_handle(),
            token,
        );
        let shell = VirtualShell::new(self.vsync.clone());
        // The engine-internal spawn path — identity is the parent-minted
        // token, never a builder option (the embedder-facing builder can
        // only ever name `ROOT`). Viewport/dpr are placeholders the host
        // element's first `flush_post_layout` rect immediately corrects via
        // a `Resize` control, fixing `viewportSize$` before the child
        // paints anything meaningful.
        let built =
            self.runtime
                .spawn_hosted_instance(token, pool, Box::new(renderer), Box::new(shell));
        match built {
            Ok((app, looper)) => {
                let child_host = app.host();
                // Load the module off the control path; the outcome ships as
                // a status event (no reply channel — see `VirtualControl`).
                let app_for_load = app;
                let tx = self.backend.worker_tx().clone();
                let wake = self.backend.worker_wake_handle();
                self.host_loop.spawn_local(Box::pin(async move {
                    let detail = match app_for_load.load_module(source).await {
                        Ok(()) => None,
                        Err(e) => Some(e.to_string()),
                    };
                    let state = if detail.is_some() {
                        VirtualStatusState::Error
                    } else {
                        VirtualStatusState::Running
                    };
                    send_status(&tx, &wake, token, state, detail);
                }));
                self.host_loop.spawn_local(Box::pin(looper.run()));
                self.children.borrow_mut().insert(token, child_host);
            }
            Err(e) => {
                send_status(
                    self.backend.worker_tx(),
                    &self.backend.worker_wake_handle(),
                    token,
                    VirtualStatusState::Error,
                    Some(format!("failed to spawn virtual app: {e}")),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ForwardingRenderer — the child's renderer
// ---------------------------------------------------------------------------

/// The child's [`Renderer`]: instead of drawing, it ships every painted
/// batch (plus image uploads stashed since the last frame) back to the
/// **parent's worker** as a [`VirtualFrameEvent`], then kicks the parent
/// worker cross-thread. Latest-wins on the parent side — this type holds no
/// frame state of its own.
pub(crate) struct ForwardingRenderer {
    parent_worker_tx: WorkerTx,
    parent_worker_wake: Rc<dyn Fn()>,
    token: VirtualAppId,
    pending_images: RefCell<HashMap<ImageResourceId, ImageResource>>,
}

impl ForwardingRenderer {
    pub(crate) fn new(
        parent_worker_tx: WorkerTx,
        parent_worker_wake: Rc<dyn Fn()>,
        token: VirtualAppId,
    ) -> Self {
        Self {
            parent_worker_tx,
            parent_worker_wake,
            token,
            pending_images: RefCell::new(HashMap::new()),
        }
    }
}

impl Renderer for ForwardingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        if commands.is_empty() {
            return;
        }
        let images = std::mem::take(&mut *self.pending_images.borrow_mut())
            .into_iter()
            .collect();
        let event = VirtualFrameEvent {
            token: self.token,
            batch: Arc::new(commands.to_vec()),
            images,
        };
        send_app_event(&self.parent_worker_tx, &self.parent_worker_wake, event);
    }

    fn upload_image_resource(&mut self, id: ImageResourceId, image: &ImageResource) {
        // Batched onto the next frame event (the child uploads exactly once
        // per id, so no dedup is needed here).
        self.pending_images.borrow_mut().insert(id, image.clone());
    }
}

impl std::fmt::Debug for ForwardingRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForwardingRenderer")
            .field("token", &self.token)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// VirtualShell — the child's shell
// ---------------------------------------------------------------------------

/// The child's [`Shell`]: hands the child the PARENT's vsync source (both
/// loopers wake on the same tick; an idle party just pumps and returns
/// `Idle` — harmless, same as today between sibling instances) and (v1)
/// drops cursor / text-input egress (translated forwarding is a later
/// milestone).
pub(crate) struct VirtualShell {
    vsync: Option<Rc<dyn VsyncSource>>,
}

impl VirtualShell {
    pub(crate) fn new(vsync: Rc<dyn VsyncSource>) -> Self {
        Self { vsync: Some(vsync) }
    }
}

impl Shell for VirtualShell {
    fn set_cursor(&mut self, _cursor: crate::core::shell::Cursor) {}

    fn request_text_input(&mut self, _state: crate::core::shell::TextInputState) {}

    fn take_vsync(&mut self) -> Option<Rc<dyn VsyncSource>> {
        self.vsync.take()
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Name of the default worker pool virtual apps spawn into. The runtime
/// builder auto-registers it (cap [`DEFAULT_POOL_MAX_WORKERS`]) unless the
/// embedder registers a pool with this name themselves.
pub(crate) const DEFAULT_POOL: &str = "virtual";

/// Default cap for the `"virtual"` pool — bounds the worker count a page
/// full of virtual views can spawn (wasm workers are the scarce resource;
/// beyond the cap, children share workers multi-tenant).
pub(crate) const DEFAULT_POOL_MAX_WORKERS: usize = 2;

// Compile-time Send assertions for everything crossing worker↔host.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<VirtualControl>();
    assert_send::<VirtualStatusEvent>();
    assert_send::<VirtualFrameEvent>();
};
