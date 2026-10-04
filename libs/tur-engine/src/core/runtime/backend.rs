//! Backend types: `WorkerBackend` (engine state on the worker thread) and
//! `HostBackend` (the host-side backend owned by `TurApp`, spawns +
//! dispatches to the worker).
//!
//! ## Architecture
//!
//! - [`WorkerBackend`] lives on the worker thread and owns the rut runtime
//!   (the loaded module's VM), the element tree, the reactive store, and
//!   the subsystems. `pump()` runs one flush and produces a
//!   `Vec<RenderCommand>` batch (stored in
//!   `TurAppInternal::pending_render_batch`).
//!
//! - [`HostBackend`] is `pub(crate)`: `TurApp` owns one. It spawns a worker
//!   (via the platform `WorkerSpawner`) hosting a `WorkerBackend`,
//!   dispatches input via `futures::channel`, and receives [`HostMsg`]
//!   replies. `HostBackend` owns the host-side [`Renderer`] (passed to
//!   `TurRuntime::app_builder().build(...)`); it applies each
//!   `HostMsg::RenderCommands` batch directly to the renderer.
//!   Embedders never touch `HostBackend` directly — everything they need
//!   is forwarded on [`TurApp`](crate::TurApp) / driven by
//!   [`TurAppLooper`](crate::TurAppLooper).
//!
//! ## Async model
//!
//! All channels use `futures::channel` (mpsc + oneshot). The platform's
//! `WorkerSpawner` drives the `async fn worker_loop(...)` future for the
//! worker's lifetime (native: the lane executor's task loop; wasm: the
//! cooperative JS-event-loop mini-executor), so the worker awaits on
//! `worker_rx.recv()` instead of blocking on a Mutex + Condvar.
//! Main-thread `TurAppLooper::run` and `rpc` are `async fn`; the embedder
//! supplies the driving executor (`wasm_bindgen_futures::spawn_local` on
//! wasm, `block_on` on the test/native caller thread).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::StreamExt;

use crate::core::app::{FrameOutcome, ModuleError, TurAppInternal, WorkerMsg};
use crate::core::app::{HostMsg, HostRx, HostTx, Reply, ShellCommand, WorkerRx, WorkerTx};
use crate::core::clock::Clock;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::{
    RenderCommand, RenderCommandBatch, Renderer, fingerprint_batch, referenced_image_ids,
};
use crate::core::scheduler::WorkerTicket;
use crate::error::TurError;

// ---------------------------------------------------------------------------
// WorkerBackend — engine state on the worker thread (pub(crate))
// ---------------------------------------------------------------------------

/// The engine state container, owned by the worker thread. Holds the rut
/// runtime (`Some` while a module is loaded), plus [`TurAppInternal`].
///
/// Constructed on the worker thread (via [`build_worker_backend`]) so it
/// can capture `!Send` types like the rut `Vm` — these never cross threads.
/// Once constructed, [`WorkerBackend::pump`] runs one flush and stores the
/// resulting `Vec<RenderCommand>` batch in
/// `TurAppInternal::pending_render_batch`, where [`HostBackend`]'s
/// `worker_loop` drains it and ships to main.
pub(crate) struct WorkerBackend {
    /// The rut runtime: `Some` while a module is loaded. Worker-side only
    /// (`Vm` is `!Send`).
    rut: RefCell<Option<crate::core::rut_runtime::RutRuntime>>,
    /// The runtime clock — the frame environment's + the rut rows' time
    /// source.
    #[allow(dead_code)]
    clock: std::sync::Arc<dyn Clock>,
    /// Worker→host sender clone — the rut trap/fuel reporter ships runtime
    /// errors to main through it, and the dev-tool RPC bridge ships
    /// `HostMsg::DevToolReply` through it when the reply transport is
    /// host-drain (see `rpc_via_host_drain`).
    host_tx: HostTx,
    /// RPC replies resolve on the awaiting (host) thread instead of via a
    /// oneshot waker fired here on the worker — required by executors
    /// whose task queues are thread-local (`wasm_bindgen_futures`).
    /// Resolved once at construction from
    /// [`WorkerExecutor::wakes_host_tasks_cross_thread`](crate::core::scheduler::WorkerExecutor::wakes_host_tasks_cross_thread).
    rpc_via_host_drain: bool,
    pub(crate) internal: TurAppInternal,
}

impl WorkerBackend {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        internal: TurAppInternal,
        clock: std::sync::Arc<dyn Clock>,
        host_tx: HostTx,
        _host_exec: crate::core::plugin::HostExecutor,
        rpc_via_host_drain: bool,
    ) -> Self {
        Self {
            rut: RefCell::new(None),
            clock,
            host_tx,
            rpc_via_host_drain,
            internal,
        }
    }

    /// Read the latest cursor applied during the last flush (or `None` if
    /// no pointer was over the surface / no cursor change happened).
    pub(crate) fn last_applied_cursor(&self) -> Option<crate::core::shell::Cursor> {
        self.internal
            .app_context
            .borrow()
            .frame_env
            .last_applied_cursor()
    }

    pub(crate) fn take_pending_render_batch(&self) -> Option<(Vec<RenderCommand>, u64)> {
        self.internal.take_pending_render_batch()
    }

    /// The rut half of the module lifecycle: best-effort `entry fn stop()`.
    /// (Root-tree teardown is shared — the auto-clear below covers both
    /// rails.)
    fn teardown_rut_module(&self) {
        if let Some(rut) = self.rut.borrow_mut().as_mut() {
            rut.stop();
        }
        *self.rut.borrow_mut() = None;
    }

    /// Parse + compile + boot a rut module and invoke its `entry fn start()`
    /// (see [`crate::core::rut_runtime`]). The parse-first contract: a
    /// broken module fails before any teardown runs.
    ///
    /// The whole path — parse, boot, `start`, and `apply_root` (the
    /// rut-built tree is pure Rust `Rc<dyn View>`) — is script-runtime-free.
    fn load_rut_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let exts = self.internal.instance.rut_pkg_exts.borrow().clone();
        crate::core::rut_runtime::RutRuntime::parse_check(source, &exts)?;

        // Module lifecycle contract: run the previous module's cleanup +
        // clear its leftover root tree before the new module runs.
        self.teardown_rut_module();

        let js = &self.internal.instance;
        let inputs = crate::core::rut_runtime::RutRealmInputs {
            clock: self.internal.app_context.borrow().frame_env.clock(),
        };
        let mut rut =
            crate::core::rut_runtime::RutRuntime::boot(source, js.clone(), inputs, exts)?;

        // Apply the root the module's `start` stashed via `tur::mount` —
        // outside the VM, realm-free (the rut-built tree is pure Rust).
        rut.apply_root().map_err(ModuleError::Eval)?;
        js.set_dirty();
        *self.rut.borrow_mut() = Some(rut);
        Ok(())
    }

    /// Engine→rut event rail: call a named `entry fn(u64, f64)`.
    fn call_rut_entry_inner(&self, name: &str, a: u64, b: f64) -> Result<(), ModuleError> {
        let mut rut_guard = self.rut.borrow_mut();
        let Some(rut) = rut_guard.as_mut() else {
            return Err(ModuleError::Eval(
                "call_rut_entry: no rut module loaded".into(),
            ));
        };
        let has_it = {
            let vm = rut.vm.borrow();
            vm.prog
                .exports
                .iter()
                .any(|(n, _)| vm.prog.interner.name(*n) == name)
        };
        if !has_it {
            return Ok(()); // no such entry — a no-op (event rails are optional)
        }
        rut.call_entry(name, a, b)?;
        // A deferred-mount entry (`mount_host(id)`-style: a host-minted id
        // crossing through the entry rail) stashes a root — apply it here,
        // the same realm-free apply the post-`start` path runs. Mount is a
        // start-time or entry-drain-time op (the mount row's face-busy
        // guard rejects mid-flush mounts); `apply_root` no-ops when the
        // entry stashed nothing.
        rut.apply_root().map_err(ModuleError::Eval)?;
        self.internal.instance.set_dirty();
        Ok(())
    }

    /// Dispatch one [`WorkerMsg`]. RPC variants settle their own `Reply`;
    /// non-RPC variants push state into the worker for the next `pump()`.
    pub(crate) fn handle_worker_msg(&self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::PlatformEvent(event) => {
                self.internal
                    .app_context
                    .borrow_mut()
                    .platform_event_queue
                    .push(event);
            }
            WorkerMsg::Wake => {
                // The worker_loop drives flush via `pump()` (separate method
                // so it can capture the FrameOutcome + ship commands).
            }
            WorkerMsg::FrameTiming { .. } => {
                // Host-side render-commit timing push-back. Intercepted by
                // `worker_loop` (record-only, no pump); this arm only keeps
                // the dispatch exhaustive.
            }
            WorkerMsg::LoadRutModule { source, reply } => {
                let res = self.load_rut_module_inner(&source);
                self.wake_if_dirty();
                reply.send(res);
            }
            WorkerMsg::CallRutEntry { name, a, b, reply } => {
                let res = self.call_rut_entry_inner(&name, a, b);
                self.wake_if_dirty();
                reply.send(res);
            }
            WorkerMsg::RutStartAnswer { reply } => {
                let answer = self
                    .rut
                    .borrow()
                    .as_ref()
                    .map(|r| r.start_answer)
                    .unwrap_or(0);
                reply.send(answer);
            }
            WorkerMsg::WithTree { runner } => {
                // Co-borrow is safe: `element_tree` and `focus_manager`
                // are distinct RefCells (the sync `focused_is_editable`
                // below borrows both the same way).
                let tree = self.internal.instance.element_tree.borrow();
                let focus = self.internal.instance.focus_manager.borrow();
                runner(&tree, &focus);
            }
            WorkerMsg::DevTool { req, reply } => {
                let json = self.dev_tool_json(req);
                if self.rpc_via_host_drain {
                    // The awaiting task lives on the host thread and its
                    // executor can't be woken from here — ride the drained
                    // host channel; `apply_msg` fires the reply on main.
                    let _ = self
                        .host_tx
                        .unbounded_send(HostMsg::DevToolReply { reply, json });
                } else {
                    reply.send(json);
                }
            }
            WorkerMsg::FrameTimingEnabled { enabled } => {
                self.internal
                    .instance
                    .frame_stats
                    .host_timing_enabled
                    .set(enabled);
            }
            WorkerMsg::AppEvent(event) => {
                self.push_app_event(event);
            }
            WorkerMsg::RegisterImageMetadata { id, size } => {
                // Host-registered image receipt: record the natural size so
                // layout + paint serve the id (`insert_with_id` — the id was
                // minted host-side, never by `ImageManager::allocate`).
                self.internal
                    .instance
                    .image_manager
                    .borrow_mut()
                    .insert_with_id(id, crate::core::image_resource::ImageMetadata { size });
            }
            WorkerMsg::Destroy { reply } => {
                // Module lifecycle contract: run the loaded module's
                // cleanup (best-effort) before the worker tears down.
                self.teardown_rut_module();
                reply.send(());
            }
        }
    }

    /// Serialize a dev-tool snapshot request against the live instance
    /// state (see `core::dev`). Shared by the worker's direct-reply path
    /// (native — the waker fires cross-thread and native executors
    /// re-poll) and the wasm bridge (`worker_loop` ships the JSON to main
    /// as [`HostMsg::DevToolReply`]; `apply_msg` resolves it there, on
    /// the thread the awaiting task was spawned on).
    pub(crate) fn dev_tool_json(&self, req: crate::core::app::DevToolRequest) -> String {
        let instance = &self.internal.instance;
        match req {
            crate::core::app::DevToolRequest::ElementTree => {
                let tree = instance.element_tree.borrow();
                crate::core::dev::element_tree_json(&tree)
            }
            crate::core::app::DevToolRequest::GetElement(id) => {
                let tree = instance.element_tree.borrow();
                crate::core::dev::get_element_json(&tree, crate::core::element::NodeId::new(id))
            }
            crate::core::app::DevToolRequest::FrameStats => {
                crate::core::dev::frame_stats_json(&instance.frame_stats)
            }
        }
    }

    pub(crate) fn pump(&self) -> Result<FrameOutcome, TurError> {
        // `Wake` is a no-op above; flush is driven here so the outcome can
        // be returned to the worker_loop, which then ships any pending
        // render batch.
        //
        // The rut VM drains FIRST — never inside a flush iteration
        // (core::rut_runtime). A no-op when no rut module is loaded.
        if let Some(rut) = self.rut.borrow_mut().as_mut() {
            rut.run_ready();
        }
        let mut outcome = self.internal.flush()?;
        // Callback intents queued during the flush (rut element callbacks)
        // drain outside the flush — a callback may read atoms and mount.
        let drained = self
            .rut
            .borrow_mut()
            .as_mut()
            .map(|rut| rut.drain_pending_calls());
        if drained.unwrap_or(0) > 0 {
            // Convergence flush: the callback's atom writes re-render here.
            // `apply_root` is realm-free (the rut-built tree is pure Rust).
            if let Some(rut) = self.rut.borrow_mut().as_mut() {
                rut.apply_root().map_err(TurError::Other)?; // a callback may re-mount
            }
            outcome = self.internal.flush()?;
        }
        Ok(outcome)
    }

    fn push_app_event(&self, event: crate::core::app::AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .app_event_queue
            .push(event);
    }

    /// After a module/script eval, re-arm an idle worker if the eval left
    /// paint-worthy state (dirty tree / `need_paint`). Coalesced + in-flush
    /// gated by `TurInstanceContext::wake_if_idle`. Lets the worker self-paint on
    /// load with no embedder paint request.
    fn wake_if_dirty(&self) {
        let js = &self.internal.instance;
        if js.dirty.get() || js.need_paint.get() {
            js.wake_if_idle();
        }
    }

    pub(crate) fn text_input_state(&self) -> crate::core::shell::TextInputState {
        crate::core::shell::TextInputState {
            is_editable: self.focused_is_editable(),
            cursor_rect: self.focused_cursor_rect(),
        }
    }

    /// The viewport the worker's `Screen` currently holds — what a batch
    /// being shipped to the host was laid out for. Stamped onto every
    /// `HostMsg::RenderCommands` so the host syncs its renderer at the
    /// render commit point (see `HostBackend::render_batch`).
    pub(crate) fn screen_viewport(&self) -> crate::core::screen::ScreenViewport {
        self.internal.app_context.borrow().screen.viewport()
    }

    pub(crate) fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.instance.focus_manager.borrow().focused()
    }

    pub(crate) fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.instance.element_tree.borrow();

        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current: Option<NodeId> = Some(focused_id.into());
        while let Some(id) = current {
            let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
            abs_x += node.computed_layout.offset.x;
            abs_y += node.computed_layout.offset.y;
            current = node.parent;
        }

        let node = tree.get_element(focused_id)?;
        let element = node.element.as_ref()?;
        let (cx, cy, cw, ch) = element.cursor_rect_relative()?;

        Some((abs_x + cx, abs_y + cy, cw, ch))
    }

    pub(crate) fn focused_is_editable(&self) -> bool {
        use crate::core::focus::helper;
        let tree = self.internal.instance.element_tree.borrow();
        let focus = self.internal.instance.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }
}

// ---------------------------------------------------------------------------
// HostBackend — TurApp's backend. Owns the worker + RPC plumbing + renderer
// ---------------------------------------------------------------------------

/// Result of dispatching one worker→host [`HostMsg`] through
/// [`HostBackend::apply_msg`] — the single message handler driven by
/// [`TurAppLooper::run`](crate::TurAppLooper::run).
///
/// `apply_msg` performs every side-effect that is independent of *when* the
/// batch is painted (cursor backend apply, focus-change handler, image
/// upload, event-bus dispatch) and returns this enum so the loop can
/// apply its vsync-aligned render policy:
/// - [`Render`] is buffered for pipelining (latest-wins) and rendered at
///   the next vsync, or flushed at quiescence if no vsync is armed.
/// - [`Frame`] fires `after_frame` and re-arms vsync when scheduled.
/// - [`Closed`] is terminal.
///
/// Because there is exactly one driver, the focus-change handler (and
/// every other side-effect) can never drift between execution paths.
pub(crate) enum MsgOutcome {
    /// Side-effects already applied; the driver should keep draining.
    Continue,
    /// A render-command batch + the viewport it was laid out for + the
    /// flush epoch it was recorded under. The driver decides when to paint
    /// it (and syncs geometry then — the render commit point); the epoch
    /// flows through so host render-commit timings are attributable.
    Render(RenderCommandBatch, crate::core::screen::ScreenViewport, u64),
    /// A completed frame. Terminal for a single-frame advance.
    Frame(FrameOutcome),
    /// The worker's flush errored. Terminal.
    Failed(String),
    /// The worker is gone. Terminal — the driver must stop.
    Closed,
}

/// The host-side backend owned by `TurApp` (shared with its
/// [`TurAppLooper`](crate::TurAppLooper)) — engine-internal, `pub(crate)`:
/// embedders reach its surface through the methods forwarded on
/// [`TurApp`](crate::TurApp). Spawns a worker thread running a
/// [`WorkerBackend`], dispatches input via `futures::channel`, and receives
/// [`HostMsg`] replies.
///
/// ## Async rpc
///
/// All rpc methods on `HostBackend` are `async fn`. The embedder
/// supplies the runtime — `wasm_bindgen_futures::spawn_local` on wasm
/// (so the JS main thread never blocks), `futures::executor::block_on`
/// on native (so the calling thread parks until the worker replies).
///
/// ## Renderer ownership
///
/// `HostBackend` owns the host-side [`Renderer`] — passed to
/// `TurRuntime::app_builder().renderer(Box<dyn Renderer>, …).build()` and
/// stored here, exactly like `main`'s
/// `app_builder().renderer(Box::new(renderer), …).build()`. Both
/// `HostBackend` and the renderer live on the main thread, so there is no
/// callback indirection: each `HostMsg::RenderCommands` batch is applied
/// directly via [`Self::render_batch`] — the render commit point, where the
/// renderer's geometry is synced to the batch's viewport immediately
/// before playback. The embedder's
/// [`TurApp::resize`](crate::TurApp::resize) (DOM `ResizeObserver` / winit
/// / JNI) only forwards the shell `Resize` event to the worker for layout
/// — the host renderer's backing store is never swapped there, so a resize
/// can never destroy the presented frame before its replacement lands (no
/// `HostMsg` round-trip either).
///
/// ## Shell egress
///
/// The worker emits `HostMsg::Shell(ShellCommand)` (cursor + text-input
/// requests, each deduped against the previous frame) alongside the
/// FrameOutcome. `apply_msg` applies them to the embedder-supplied
/// [`Shell`](crate::core::shell::Shell) — installed at construction via
/// [`TurAppBuilder::shell`](crate::core::runtime::TurAppBuilder::shell)
/// (default [`NoopShell`](crate::core::shell::NoopShell)). The engine
/// retains no cursor / text-input cache on the host side.
pub(crate) struct HostBackend {
    worker_tx: WorkerTx,
    /// Holds the app's worker-slot claim alive for the backend's
    /// lifetime so the hosting worker doesn't reclaim the slot.
    _worker_ticket: WorkerTicket,
    /// Cross-thread wake. Called after every host→worker send. No-op on
    /// native; `worker.postMessage(0)` on wasm.
    worker_wake: Rc<dyn Fn()>,
    /// The embedder-supplied shell — the per-instance host-side OS
    /// interaction surface (cursor output + text-input requests). Supplied
    /// at construction via `TurAppBuilder::shell`; owned exclusively here,
    /// so `apply_msg` can apply commands with a plain `borrow_mut()`.
    shell: RefCell<Box<dyn crate::core::shell::Shell>>,
    /// Main-side renderer (owned — no sink callback). Worker ships
    /// `HostMsg::RenderCommands` batches; main applies them here. `None`
    /// while the instance is **detached** (built without a renderer, or
    /// between [`TurApp::detach_renderer`] and
    /// [`TurApp::attach_renderer`]): every render-side call (batch
    /// application, present, image upload, resize, readback) skips
    /// silently — the engine loop keeps running, only the GPU output is
    /// gone. This is the two-phase (initialize → attach) lifecycle seam:
    /// the Android embedder builds its instances renderer-less and attaches
    /// a surface renderer when the platform surface exists.
    renderer: RefCell<Option<Box<dyn Renderer>>>,
    /// Main-side image resources — the full `ImageResource` (pixel `Blob`
    /// retained) per worker-assigned id. Inserted on `HostMsg::UploadImage`
    /// (under the worker-assigned id) alongside the GPU upload; re-ensured
    /// at the render commit point for every id a painted frame references
    /// ([`Self::ensure_batch_images`]) — the retention exists precisely so
    /// an empty atlas (fresh attach, surface recreation) repopulates at
    /// first paint. The worker only ever holds the sizes (`ImageManager`).
    image_resource_map: RefCell<crate::core::image_resource::ImageResourceMap>,
    /// Next host-minted image id, counting DOWN from
    /// [`HOST_IMAGE_ID_BASE`](crate::core::image_resource::HOST_IMAGE_ID_BASE)
    /// (disjoint from the worker's up-counting range; every value stays
    /// f64-exact for the JS number boundary). Consumed by
    /// [`Self::register_image`].
    next_host_image_id: Cell<u64>,
    /// The last viewport actually applied to the owned renderer. The render
    /// commit point (`render_batch`) syncs geometry to each batch's
    /// viewport, deduped against this — a steady frame stream never
    /// reconfigures the surface, and a resize is applied exactly once (with
    /// the first frame painted for it). `None` until the first sync.
    last_viewport: Cell<Option<crate::core::screen::ScreenViewport>>,
    /// Host-side frame-timing collection gate — set by
    /// `HostMsg::FrameTimingEnabled` (the `turDevTool.setHostFrameTiming`
    /// bridge). While on, every applied frame's render-commit timings ship
    /// back via `WorkerMsg::FrameTiming` (no wake — record-only on the
    /// worker). Off by default: zero per-frame overhead when unused.
    frame_timing_enabled: Cell<bool>,
    /// Content fingerprint of the last APPLIED frame — the frame-dedup
    /// signal. `None` after attach/detach (a fresh renderer has painted
    /// nothing, so the next batch must always apply). Skips scene rebuild +
    /// re-encode + re-raster when a new batch carries identical content
    /// (see [`Self::render_batch`]).
    last_frame_fingerprint: Cell<Option<u64>>,
    /// The runtime clock — times the host-side render-commit phases for
    /// the frame-timing probe. Held as an `Arc` clone;
    /// `std::time::Instant` is unavailable on wasm.
    clock: std::sync::Arc<dyn Clock>,
}

impl HostBackend {
    /// Host an app loop in `worker_pool` via the runtime's
    /// [`WorkerSpawner`](crate::core::scheduler::WorkerSpawner). The entry runs
    /// on the chosen worker (lane thread / Web Worker — platform-defined)
    /// and constructs the [`WorkerBackend`] (so it can build `!Send` types
    /// like the rut `Vm`).
    ///
    /// The platform hands the entry a
    /// [`WorkerContext`](crate::core::scheduler::WorkerContext) for that
    /// worker and then drives the returned future (the engine's
    /// `worker_loop`) for the worker's lifetime. The entry also receives
    /// a worker→host channel sender clone (`host_tx`) so bridges can ship
    /// messages (e.g. `HostMsg::UploadImage` from `createImageResource`)
    /// directly without a staging vec.
    ///
    /// Readiness follows the spawner's
    /// [contract](crate::core::scheduler::WorkerSpawner::spawn_worker):
    /// blocking implementations return only after the entry's synchronous
    /// prologue completed; wasm returns immediately and embedders confirm
    /// via the first RPC await.
    ///
    /// Returns the backend **plus** the worker→host message stream
    /// (`HostRx`) it spawned — the receiving half belongs to the
    /// instance's [`TurAppLooper`](crate::TurAppLooper) (the autonomous
    /// loop), which drains it exclusively, so the backend itself keeps
    /// only the sending-side plumbing.
    pub(crate) fn new(
        worker_spawner: Rc<dyn crate::core::scheduler::WorkerSpawner>,
        clock: std::sync::Arc<dyn Clock>,
        renderer: Option<Box<dyn Renderer>>,
        shell: Box<dyn crate::core::shell::Shell>,
        worker_pool: crate::core::scheduler::WorkerPoolHandle,
        backend_factory: impl FnOnce(
            crate::core::scheduler::WorkerContext,
            std::sync::Arc<dyn Fn() + Send + Sync>,
            crate::core::app::HostTx,
        ) -> WorkerBackend
        + Send
        + 'static,
    ) -> (Self, HostRx) {
        let (worker_tx, worker_rx) = futures::channel::mpsc::unbounded::<WorkerMsg>();
        let (host_tx, host_rx) = futures::channel::mpsc::unbounded::<HostMsg>();

        let worker_tx_for_on_push = worker_tx.clone();
        // Clone of the worker→host sender handed to the backend so bridges
        // can ship messages directly (FIFO order is preserved across the
        // shared channel — the bridge enqueues during flush, worker_loop
        // enqueues after flush).
        let main_tx_for_backend = host_tx.clone();
        let worker_ticket = worker_spawner.spawn_worker(
            &worker_pool,
            Box::new(move |worker_ctx| {
                let worker_tx_for_on_push = worker_tx_for_on_push.clone();
                // `Send + Sync` so the flush-driven task waker (which sleep
                // futures register with the test `VirtualClock`, fired
                // cross-thread) can hold an `Arc` clone.
                let wake_worker: std::sync::Arc<dyn Fn() + Send + Sync> =
                    std::sync::Arc::new(move || {
                        let _ = worker_tx_for_on_push.unbounded_send(WorkerMsg::Wake);
                    });
                let backend = backend_factory(worker_ctx.clone(), wake_worker, main_tx_for_backend);
                // Return the worker's main future; the platform drives it
                // for the worker's lifetime.
                Box::pin(worker_loop(backend, worker_rx, host_tx))
            }),
        );

        // Cross-thread wake callback. No-op on native (mpsc waker unparks
        // the OS thread); `worker.postMessage(0)` on wasm (the only way to
        // kick an idle Web Worker's JS event loop without a sync
        // `Atomics.wait`). Called after every host→worker send below.
        let worker_wake = worker_ticket.wake();

        (
            Self {
                worker_tx: worker_tx.clone(),
                _worker_ticket: worker_ticket,
                worker_wake,
                shell: RefCell::new(shell),
                renderer: RefCell::new(renderer),
                image_resource_map: RefCell::new(
                    crate::core::image_resource::ImageResourceMap::default(),
                ),
                next_host_image_id: Cell::new(crate::core::image_resource::HOST_IMAGE_ID_BASE),
                last_viewport: Cell::new(None),
                frame_timing_enabled: Cell::new(false),
                last_frame_fingerprint: Cell::new(None),
                clock,
            },
            host_rx,
        )
    }

    /// The cross-thread wake for this instance's worker (`Rc<dyn Fn()>`,
    /// host-thread-only). Virtual-app hosting (frames / status back into
    /// the parent's worker) reuses it after every send.
    pub(crate) fn worker_wake_handle(&self) -> Rc<dyn Fn()> {
        self.worker_wake.clone()
    }

    /// Send a fire-and-forget [`WorkerMsg`] (no Reply slot). Used by
    /// `push_platform_event`, `push_app_event`,
    /// `emit_to_js`. Wakes the worker cross-thread after the send (no-op on
    /// native; `postMessage` on wasm).
    pub(crate) fn send_worker_msg(&self, msg: WorkerMsg) {
        let _ = self.worker_tx.unbounded_send(msg);
        self.wake_worker();
    }

    /// Fire the cross-thread wake callback. Cheap; safe to call after every
    /// host→worker send. `(&*rc)()` derefs the `Rc<dyn Fn>` to call it.
    fn wake_worker(&self) {
        use std::ops::Deref;
        self.worker_wake.deref()();
    }

    /// Apply a render-command batch to the owned renderer (geometry sync +
    /// image re-ensure + encode + present) — the **render commit point**.
    /// Called from `TurAppLooper::run` (both the vsync-aligned pipelining
    /// path and the quiescence flush) — single source of truth for render
    /// application.
    ///
    /// Geometry is synced FIRST (deduped via [`Self::sync_viewport`]), so
    /// the backing-store swap and this frame's content land in one
    /// operation: the renderer's currently-presented frame is never
    /// destroyed by a resize whose replacement frame hasn't arrived yet
    /// (which is what resizing at event-receipt time did — the resize white
    /// flash). Then every image the batch references is re-ensured into the
    /// atlas (idempotent — see [`Self::ensure_batch_images`]) BEFORE
    /// playback, so a paint can never hit a mapped-id miss. A no-op while
    /// detached (`None` slot).
    ///
    /// **Frame dedup:** a batch whose content fingerprint matches the last
    /// APPLIED frame's (same content + viewport; the presented surface
    /// already shows this exact frame) skips scene rebuild, re-encode, and
    /// re-raster entirely. The fingerprint resets on renderer attach/detach
    /// — a fresh renderer has never painted anything, so the next batch
    /// must always apply.
    ///
    /// When frame timing is enabled, the apply (scene rebuild + playback)
    /// and present (encode + raster + composite) phases are timed and the
    /// results ship back to the worker via `WorkerMsg::FrameTiming`
    /// (record-only — no wake, no flush feedback).
    pub(crate) fn render_batch(
        &self,
        commands: &[RenderCommand],
        viewport: crate::core::screen::ScreenViewport,
        frame_id: u64,
    ) {
        self.sync_viewport(viewport);
        let mut renderer = self.renderer.borrow_mut();
        let Some(r) = renderer.as_mut() else {
            return;
        };

        // Frame dedup: identical content + identical viewport ⇒ the
        // presented frame already shows it — skip the whole pipeline.
        let fingerprint = fingerprint_batch(commands, &viewport);
        if self.last_frame_fingerprint.get() == Some(fingerprint) {
            return;
        }

        let timing = self.frame_timing_enabled.get();
        let apply_start = timing.then(|| crate::core::app::frame_stats::clock_now_us(&*self.clock));
        self.ensure_batch_images(r.as_mut(), commands);
        r.as_mut().render_commands(commands);
        let apply_us = apply_start
            .map(|start| {
                crate::core::app::frame_stats::clock_now_us(&*self.clock).saturating_sub(start)
            })
            .unwrap_or(0);
        let present_start =
            timing.then(|| crate::core::app::frame_stats::clock_now_us(&*self.clock));
        let _ = r.present();
        let present_us = present_start
            .map(|start| {
                crate::core::app::frame_stats::clock_now_us(&*self.clock).saturating_sub(start)
            })
            .unwrap_or(0);
        drop(renderer);
        self.last_frame_fingerprint.set(Some(fingerprint));
        if timing {
            let _ = self.worker_tx.unbounded_send(WorkerMsg::FrameTiming {
                frame_id,
                apply_us,
                present_us,
            });
            // No wake: a busy worker delivers within the frame stream; an
            // idle worker doesn't need this message (and must not flush on
            // it).
        }
    }

    /// Ensure the renderer's atlas holds every image this batch is about to
    /// paint — the lazy half of retention: registration uploads eagerly,
    /// and the render commit point re-ensures (idempotently; renderers
    /// dedupe), so an empty atlas (fresh attach, anything that skipped the
    /// registration rail) can never paint a mapped id blank. Only ids the
    /// frame actually references are ensured — pay for what you paint. A
    /// no-op when nothing is retained. `r` arrives through the caller's
    /// held `RefMut` — going through `self.upload_image_resource` here
    /// would double-borrow `self.renderer`.
    fn ensure_batch_images(&self, r: &mut dyn Renderer, commands: &[RenderCommand]) {
        if self.image_resource_map.borrow().is_empty() {
            return;
        }
        for id in referenced_image_ids(commands) {
            if let Some(image) = self.image_resource_map.borrow().get_image(id) {
                r.upload_image_resource(id, image);
            }
        }
    }

    /// Sync the owned renderer to the given viewport — the geometry half of
    /// the render commit point. Deduped against the last viewport actually
    /// applied ([`Self::last_viewport`]), so a steady frame stream never
    /// reconfigures the surface and a resize is applied exactly once — with
    /// the first frame painted for it. A no-op on the renderer itself while
    /// detached (the shell `Resize` event still reached the worker, so
    /// `viewportSize$` tracks the size; the next attached renderer is
    /// synced by its explicit attach, and the next batch re-syncs).
    pub(crate) fn sync_viewport(&self, viewport: crate::core::screen::ScreenViewport) {
        if self.last_viewport.get() == Some(viewport) {
            return;
        }
        if let Some(r) = self.renderer.borrow_mut().as_mut() {
            r.resize(
                viewport.logical_width,
                viewport.logical_height,
                viewport.dpr,
            );
        }
        self.last_viewport.set(Some(viewport));
    }

    /// Upload a newly-registered image resource to the owned renderer (a
    /// no-op while detached — the resource stays retained in the host-side
    /// map, and the render commit point re-ensures it before the next frame
    /// that paints it).
    pub(crate) fn upload_image_resource(&self, id: ImageResourceId, image: &ImageResource) {
        if let Some(r) = self.renderer.borrow_mut().as_mut() {
            r.upload_image_resource(id, image);
        }
    }

    /// Retain a shipped image resource on main (under the worker-assigned
    /// id) — the host-side `ImageResourceMap` is the pixel `Blob` owner,
    /// kept for context-loss re-upload. The worker never retains the Blob.
    pub(crate) fn insert_image_resource(&self, id: ImageResourceId, image: ImageResource) {
        self.image_resource_map
            .borrow_mut()
            .insert_with_id(id, image);
    }

    /// Retain + upload — the shared body of the `HostMsg::UploadImage` arm
    /// (worker-decoded images) and [`Self::register_image`] (host-registered
    /// images): the full resource is retained host-side (re-ensured at the
    /// render commit point before any frame that paints it — see
    /// [`Self::ensure_batch_images`]), then uploaded into the GPU atlas (a
    /// no-op while detached).
    fn retain_and_upload_image(&self, id: ImageResourceId, image: &ImageResource) {
        self.insert_image_resource(id, image.clone());
        self.upload_image_resource(id, image);
    }

    /// Register a host-formed image resource: mint an id from the host range
    /// (counting down from
    /// [`HOST_IMAGE_ID_BASE`](crate::core::image_resource::HOST_IMAGE_ID_BASE)),
    /// retain the pixel Blob + upload it to the renderer (identical rail to
    /// the `UploadImage` arm — no `HostMsg` needed, we ARE the host thread),
    /// and notify the worker with just the natural size via
    /// `WorkerMsg::RegisterImageMetadata` so layout + paint can serve the id.
    ///
    /// The pixel bytes never cross to the worker — JS references the image
    /// through the returned handle (a plain number once it crosses the JS
    /// boundary, wrapped into an `ImageResourceHandle` by the
    /// `imageResourceHandle` bridge). The FIFO worker channel guarantees the
    /// worker records the metadata before any later message can expose the
    /// id to JS. Host-thread method.
    pub(crate) fn register_image(&self, image: ImageResource) -> ImageResourceId {
        let id = ImageResourceId::new(self.next_host_image_id.get());
        self.next_host_image_id.set(id.as_u64() - 1);
        self.retain_and_upload_image(id, &image);
        self.send_worker_msg(WorkerMsg::RegisterImageMetadata {
            id,
            size: image.natural_size,
        });
        id
    }

    /// Install (or replace) the renderer — the **attach** half of the
    /// two-phase lifecycle. A bare install: the fresh renderer's atlas
    /// repopulates lazily — the render commit point re-ensures every image
    /// a frame references before painting it (see
    /// [`Self::render_batch`]). Host-thread method (same discipline as
    /// [`Self::sync_viewport`]). See [`TurApp::attach_renderer`].
    pub(crate) fn attach_renderer(&self, renderer: Box<dyn Renderer>) {
        // A fresh renderer has never painted anything — reset the frame
        // dedup signal so the next batch always applies (and its atlas
        // repopulates via the image re-ensure).
        self.last_frame_fingerprint.set(None);
        *self.renderer.borrow_mut() = Some(renderer);
    }

    /// Take + drop the renderer — the **detach** half. Host-thread method.
    /// Idempotent (detaching an already-detached instance is a no-op). See
    /// [`TurApp::detach_renderer`].
    pub(crate) fn detach_renderer(&self) {
        // Same as attach: the dedup signal describes the OLD renderer's
        // presented content; the next renderer must paint from scratch.
        self.last_frame_fingerprint.set(None);
        self.renderer.borrow_mut().take();
    }

    /// Pixel readback from the owned renderer (screenshot tests). Returns
    /// `None` while detached or if the renderer doesn't support readback.
    pub(crate) fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.renderer
            .borrow_mut()
            .as_mut()
            .and_then(|r| r.render_to_pixels())
    }

    /// Borrow the worker→host channel sender. Used by call sites that
    /// build a `WorkerMsg` carrying a closure / reply slot directly (e.g.
    /// [`TurApp::with_tree`](crate::TurApp::with_tree)).
    /// Toggle host-side frame-timing collection (the dev tool's
    /// `setHostFrameTiming`): gates the per-frame `WorkerMsg::FrameTiming`
    /// push-back.
    pub(crate) fn set_frame_timing_enabled(&self, enabled: bool) {
        self.frame_timing_enabled.set(enabled);
    }

    pub(crate) fn worker_tx(&self) -> &WorkerTx {
        &self.worker_tx
    }

    /// The single worker→host message handler. Pure dispatch + side-effects
    /// for rendering policy: `RenderCommands` is handed back as
    /// [`MsgOutcome::Render`] so the loop can buffer it for vsync-aligned
    /// pipelining. All backend mutations (`shell`,
    /// image uploads, event-bus dispatch) happen
    /// here.
    pub(crate) fn apply_msg(&self, msg: HostMsg) -> MsgOutcome {
        match msg {
            HostMsg::RenderCommands {
                commands,
                viewport,
                frame_id,
            } => MsgOutcome::Render(commands, viewport, frame_id),
            HostMsg::UploadImage { id, image } => {
                // Retain the full resource (pixel Blob) on main for
                // context-loss re-upload, then upload into the GPU atlas.
                self.retain_and_upload_image(id, &image);
                MsgOutcome::Continue
            }
            HostMsg::Shell(cmd) => {
                let mut shell = self.shell.borrow_mut();
                match cmd {
                    ShellCommand::SetCursor(cursor) => shell.set_cursor(cursor),
                    ShellCommand::RequestTextInput(state) => shell.request_text_input(state),
                }
                MsgOutcome::Continue
            }
            HostMsg::FrameOutcome(Ok(outcome)) => MsgOutcome::Frame(outcome),
            HostMsg::FrameOutcome(Err(e)) => MsgOutcome::Failed(e),
            HostMsg::Destroyed => MsgOutcome::Closed,
            HostMsg::FrameTimingEnabled(on) => {
                self.frame_timing_enabled.set(on);
                MsgOutcome::Continue
            }
            HostMsg::DevToolReply { reply, json } => {
                // Resolve on MAIN — the whole point of the bridge (see the
                // variant doc): the awaiting dev-tool task is a main-thread
                // `wasm_bindgen_futures` task, so its waker must fire here.
                reply.send(json);
                MsgOutcome::Continue
            }
            // Virtual-app controls are routed by `TurAppLooper` — the drain
            // point — directly to the instance's `VirtualHost` core (the
            // host-side core shared by the app facade + looper; the backend
            // holds none of it). This backend never sees one; the arm exists
            // only to keep the match exhaustive.
            HostMsg::VirtualControl(_) => {
                unreachable!("HostMsg::VirtualControl is routed by TurAppLooper before apply_msg")
            }
            // Same for runtime-error reports — forwarded to the parent
            // worker (children) or logged (root) by the looper.
            HostMsg::RuntimeError { .. } => {
                unreachable!("HostMsg::RuntimeError is routed by TurAppLooper before apply_msg")
            }
        }
    }

    /// RPC dispatch — send a [`WorkerMsg`] with a Reply slot, await the
    /// reply. Async: the caller (e.g. `eval_js`) is itself `async fn`;
    /// the embedder drives it via its runtime.
    pub(crate) async fn rpc<T: 'static>(
        &self,
        msg_builder: impl FnOnce(crate::core::app::ReplySender<T>) -> WorkerMsg,
    ) -> T {
        let (tx, rx) = Reply::<T>::pair();
        let msg = msg_builder(tx);
        let _ = self.worker_tx.unbounded_send(msg);
        self.wake_worker();
        rx.rx.await.expect("reply sender dropped without firing")
    }

    /// Module load — the RPC entry behind [`TurApp::load_rut_module`].
    ///
    /// Cancellation-tolerant by design: the caller can be an element-hosted
    /// child whose host was destroyed while the load was in flight (the
    /// layout-tab switch retires the live child and spawns its replacement
    /// in one flush — the retiring child's worker loop exits on `Destroy`
    /// and drops the RPC inbox, canceling this reply). A canceled reply is
    /// that lifecycle event, so it surfaces as
    /// [`ModuleError::WorkerGone`] — NOT the `rpc` invariant panic (which
    /// on wasm is a `panic = "abort"` trap that took the whole engine
    /// down: the Edit→Split playground crash).
    pub(crate) async fn load_rut_module(
        &self,
        source: impl Into<std::sync::Arc<str>>,
    ) -> Result<(), ModuleError> {
        let source = source.into();
        tracing::info!("load_rut_module: booting module ({} bytes)", source.len());
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        let _ = self
            .worker_tx
            .unbounded_send(WorkerMsg::LoadRutModule { source, reply: tx });
        self.wake_worker();
        match rx.rx.await {
            Ok(res) => res,
            // The reply sender dropped without firing — the instance's
            // worker is gone (destroyed mid-load). The load's outcome is
            // moot; report the gone worker instead of aborting.
            Err(_) => Err(ModuleError::WorkerGone),
        }
    }

    /// Engine→rut event rail — the RPC entry behind
    /// [`TurApp::call_rut_entry`].
    pub(crate) async fn call_rut_entry(
        &self,
        name: &str,
        a: u64,
        b: f64,
    ) -> Result<(), ModuleError> {
        let name = std::sync::Arc::from(name);
        self.rpc(|tx| WorkerMsg::CallRutEntry {
            name,
            a,
            b,
            reply: tx,
        })
        .await
    }

    /// The loaded rut module's `entry fn start() -> u64` answer.
    pub(crate) async fn rut_start_answer(&self) -> u64 {
        self.rpc(|tx| WorkerMsg::RutStartAnswer { reply: tx }).await
    }


    /// Count of image resources retained on main (pixel `Blob`s). Test-only
    /// introspection (forwarded on
    /// [`TurApp::image_resource_count`](crate::TurApp::image_resource_count)):
    /// asserts `HostMsg::UploadImage` was received (shipped
    /// directly from the `createImageResource` bridge) and inserted into
    /// main's `ImageResourceMap`.
    pub(crate) fn image_resource_count(&self) -> usize {
        self.image_resource_map.borrow().iter_images().count()
    }
}

/// Worker loop. Runs as `async fn`, driven for the worker's lifetime by
/// the platform's `WorkerSpawner` (native: the lane executor; wasm: the
/// worker's cooperative JS-event-loop mini-executor).
///
/// Awaits on `worker_rx.recv()` for incoming `WorkerMsg`s. On `Wake`,
/// pumps the engine (`backend.pump()`), then ships:
/// 1. `HostMsg::RenderCommands` (if the flush painted)
/// 2. `HostMsg::FrameOutcome` (always)
/// 3. `HostMsg::Shell(ShellCommand)` (cursor + text-input, each deduped)
///
/// `HostMsg::UploadImage` is **not** shipped here — decoded images are
/// shipped directly from the `createImageResource` bridge via the shared
/// `host_tx` clone held in `TurInstanceContext` (one ship per decode, FIFO).
/// `HostMsg::Resized` does not exist — geometry travels inside
/// `HostMsg::RenderCommands` (the batch's viewport), so the host resizes
/// its renderer only at the render commit point, atomically with the frame
/// painted for that size.
///
/// All other variants (`PlatformEvent`, RPCs) are
/// dispatched to `backend.handle_worker_msg` (RPC variants fire their own
/// `ReplySender`).
async fn worker_loop(backend: WorkerBackend, mut worker_rx: WorkerRx, host_tx: HostTx) {
    let mut last_cursor: Option<crate::core::shell::Cursor> = None;
    type FocusCache = Option<(bool, Option<(f64, f64, f64, f64)>)>;
    let mut last_focus: FocusCache = None;
    while let Some(msg) = worker_rx.next().await {
        match msg {
            // `Wake` is a bare flush request (dispatch no-op); an
            // `AppEvent` queues an engine-internal event whose consumers
            // all live inside `flush()`'s subsystem drain — and it may
            // arrive while the instance is otherwise idle (no vsync armed,
            // no input in flight), so it must drive its own flush rather
            // than wait for the next `Wake` (virtual-app frames stalled
            // without this: child resize → child repaint → frame event
            // queued → never drained → stale replay).
            msg @ (WorkerMsg::Wake | WorkerMsg::AppEvent(_)) => {
                backend.handle_worker_msg(msg);
                let outcome = backend.pump();
                let payload = match outcome {
                    Ok(fo) => Ok(fo),
                    Err(e) => {
                        tracing::error!("worker pump error: {e}");
                        Err(e.to_string())
                    }
                };
                // Ship render commands if the flush painted — stamped with
                // the viewport they were laid out for, so the host syncs its
                // renderer at the render commit point (see
                // `HostBackend::render_batch`).
                if let Some((batch, frame_id)) = backend.take_pending_render_batch() {
                    let viewport = backend.screen_viewport();
                    let _ = host_tx.unbounded_send(HostMsg::RenderCommands {
                        commands: batch,
                        viewport,
                        frame_id,
                    });
                }
                let _ = host_tx.unbounded_send(HostMsg::FrameOutcome(payload));
                // Ship cursor changes (deduped against the last emitted).
                let current_cursor = backend.last_applied_cursor();
                if current_cursor != last_cursor {
                    last_cursor = current_cursor;
                    let _ = host_tx.unbounded_send(HostMsg::Shell(ShellCommand::SetCursor(
                        current_cursor.unwrap_or_default(),
                    )));
                }
                // Ship text-input state changes (deduped against the last
                // emitted).
                let current_focus = backend.text_input_state();
                let focus_key = (current_focus.is_editable, current_focus.cursor_rect);
                if Some(focus_key) != last_focus {
                    last_focus = Some(focus_key);
                    let _ = host_tx.unbounded_send(HostMsg::Shell(ShellCommand::RequestTextInput(
                        current_focus,
                    )));
                }
            }
            WorkerMsg::FrameTiming {
                frame_id,
                apply_us,
                present_us,
            } => {
                // Host-side render-commit timing push-back (opt-in).
                // Record-only — deliberately no pump: this message must not
                // drive a flush (it would create a render↔timing feedback
                // loop). A busy worker delivers it within the current frame
                // stream; an idle worker doesn't need it.
                backend.internal.instance.frame_stats.record_host_timing(
                    crate::core::app::frame_stats::HostFrameTiming {
                        frame_id,
                        apply_us,
                        present_us,
                    },
                );
            }
            msg @ WorkerMsg::Destroy { .. } => {
                // Module lifecycle contract: the loaded module's cleanup
                // (best-effort) runs in the shared dispatch arm
                // (`teardown_current_module`) before its reply fires —
                // routing through `handle_worker_msg` (instead of
                // intercepting here) keeps a single implementation of
                // destroy-time teardown and un-deadcodes that arm. Then the
                // loop ships `Destroyed` and exits.
                backend.handle_worker_msg(msg);
                let _ = host_tx.unbounded_send(HostMsg::Destroyed);
                break;
            }
            // All other variants (PlatformEvent, LoadModule,
            // RPCs) delegate to the worker dispatch — RPC
            // variants fire their own ReplySender.
            other => backend.handle_worker_msg(other),
        }
    }
}
