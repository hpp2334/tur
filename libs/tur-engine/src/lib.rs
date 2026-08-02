pub mod builtin_plugins;
pub mod core;
pub mod renderer;

pub mod error;

// Re-export engine-internal cursor-capability types at the crate root so
// embedders can write `tur_engine::CursorBackend`, `tur_engine::CursorCap`,
// `tur_engine::NoopCursor` without reaching into `core::platform`. The std
// plugin itself (`TurStdPlugin`) lives in `builtin_plugins::std`.
pub use crate::core::platform::{CursorBackend, CursorCap, NoopCursor};
// Re-export the clipboard plugin surface at the crate root so embedders /
// external backend crates can write `tur_engine::Clipboard`,
// `tur_engine::ClipboardBackend`, `tur_engine::TurClipboardPlugin`,
// `tur_engine::platform_paste`. The plugin itself lives in
// `builtin_plugins::clipboard` (inlined from the former
// `tur-clipboard-capability` crate).
pub use crate::builtin_plugins::clipboard::{
    Clipboard, ClipboardBackend, TurClipboardPlugin, platform_paste,
};
// Re-export `TurStdPlugin` at the crate root so embedders can write
// `tur_engine::TurStdPlugin` (was previously in a separate `tur-std` crate).
pub use crate::builtin_plugins::TurStdPlugin;
pub use crate::builtin_plugins::event_bus::EventBus;
// Re-export the runtime + builder at the crate root — the primary entry point
// for embedders. `TurRuntime::builder()` is the shared, created-once object;
// `runtime.create_app()` / `runtime.create_headless_app()` spawn isolated
// `TurApp` instances.
pub use crate::core::runtime::{TurRuntime, TurRuntimeBuilder};

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use boa_engine::Context;
use boa_engine::Source;

use error::TurError;

use core::app::{FrameOutcome, ModuleError, Reply, TurAppInternal, WorkerMsg};
use core::async_::TurJobExecutor;
use core::element::{ElementNodeId, NodeId};
use core::elements::AnyElement;

#[cfg(feature = "trace")]
use core::elements::NodeTreeData;

pub struct TurApp {
    boa_context: RefCell<Context>,
    internal: TurAppInternal,
    executor: Rc<TurJobExecutor>,
    /// Autonomous-loop driver. `None` until [`Self::start`] is called
    /// (production); tests leave it unset and pump via [`Self::run_frame`].
    driver: RefCell<Option<Rc<dyn LoopDriver>>>,
    /// Long-lived wake trampoline created in [`Self::start`]: upgrades a
    /// `Weak<Self>` and calls [`Self::wake`]. Held here (and cloned into the
    /// driver via [`LoopDriver::set_wake`]) so it stays alive for the loop's
    /// lifetime; the `Weak` back-ref avoids a reference cycle.
    wake_fn: RefCell<Option<Rc<dyn Fn()>>>,
    /// Embedder-installed callback fired after each autonomous frame — used by
    /// the wasm embedder for DOM side-effects (file-pick resolution, textarea
    /// focus / caret positioning). `None` in tests.
    after_frame: RefCell<Option<AfterFrameHook>>,
}

/// Per-frame hook fired at the end of [`TurApp::wake`] (after `run_frame`,
/// before rescheduling). See [`TurApp::set_after_frame_hook`].
pub type AfterFrameHook = Rc<dyn Fn(FrameOutcome)>;

/// Snapshot of focused-element state — single struct for the two-value
/// `focused_is_editable` + `focused_cursor_rect` pair. Used by
/// [`TurApp::focused_state`]. Phase 7's worker emits the equivalent
/// `MainMsg::FocusedStateChanged` on change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusedState {
    pub is_editable: bool,
    /// Logical-space `(x, y, w, h)` of the focused element's caret, or
    /// `None` if no editable is focused.
    pub cursor_rect: Option<(f64, f64, f64, f64)>,
}

impl TurApp {
    pub fn load_js(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        self.handle_worker_msg(WorkerMsg::LoadJs {
            source: Arc::from(source),
            reply: tx,
        });
        rx.try_recv()
            .ok_or_else(|| TurError::Other("worker gone".into()))
            .and_then(|res| res.map_err(TurError::from))
    }

    pub fn load_module(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_module: evaluating module ({} bytes)", source.len());
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        self.handle_worker_msg(WorkerMsg::LoadModule {
            source: Arc::from(source),
            reply: tx,
        });
        rx.try_recv()
            .ok_or_else(|| TurError::Other("worker gone".into()))
            .and_then(|res| res.map_err(TurError::from))
    }

    pub fn eval_module(&self, source: &str) -> Result<(), TurError> {
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        self.handle_worker_msg(WorkerMsg::EvalModule {
            source: Arc::from(source),
            reply: tx,
        });
        rx.try_recv()
            .ok_or_else(|| TurError::Other("worker gone".into()))
            .and_then(|res| res.map_err(TurError::from))
    }

    /// Advance exactly one frame: run the engine's fixed-point flush (events,
    /// reactive updates, layout, microtasks, async polling) and render if
    /// anything changed. Returns the outcome including how the next frame
    /// should be scheduled.
    ///
    /// This is the canonical frame entry. `pump` and `run_frame` are
    /// aliases; `pump` matches the worker/main vocabulary used in Phase 4+
    /// (the main thread "pumps" the worker task once per rAF).
    ///
    /// Embedders normally drive the engine via [`Self::start`] (autonomous
    /// loop); test harnesses and advanced embedders call this directly.
    ///
    /// Unlike the old `spawn_loop_once`, this takes no time argument — the
    /// clock is the engine's own `Clock` (a real wall-clock in production,
    /// a `FixedClock` the harness advances in tests).
    pub fn pump(&self) -> Result<core::app::FrameOutcome, TurError> {
        // Phase 4 single-threaded: the Wake handler ran flush inline and
        // emitted MainMsg::FrameOutcome to the (yet-unwired) main channel.
        // We return the outcome directly; Phase 7's pump will drain a real
        // mpsc until it sees FrameOutcome.
        self.handle_worker_msg(WorkerMsg::Wake);
        let mut boa = self.boa_context.borrow_mut();
        self.internal.flush(&mut boa)
    }

    /// Legacy alias for [`Self::pump`]. Kept for embedder/test back-compat
    /// during the Phase 4 transition; new code should call `pump`.
    pub fn run_frame(&self) -> Result<core::app::FrameOutcome, TurError> {
        self.pump()
    }

    pub fn with_boa_context<R>(&self, f: impl FnOnce(&mut Context) -> R) -> R {
        f(&mut self.boa_context.borrow_mut())
    }

    /// Retrieve per-instance typed data stored by a plugin during `register`
    /// via [`PluginContext::store_instance_data](core::plugin::PluginContext::store_instance_data).
    /// Returns `None` if no plugin stored data of type `T`.
    ///
    /// Each plugin exposes its own `of()` wrapper around this (e.g.
    /// `EventBus::of(&app)`).
    pub fn instance_data<T: 'static>(&self) -> Option<Rc<T>> {
        self.internal
            .instance_data
            .borrow()
            .get(&std::any::TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<Rc<T>>())
            .cloned()
    }

    /// Always-installed event bus handle (Phase 5+). The bus is
    /// unconditionally registered by `TurStdPlugin`, so this never fails —
    /// the historical `EventBus::of(&app) -> Option<EventBus>` was always
    /// `Some`. New code should prefer this direct accessor.
    ///
    /// Phase 5 keeps `EventBus::of` as a back-compat alias (returns
    /// `Some(self.event_bus())`).
    pub fn event_bus(&self) -> builtin_plugins::event_bus::EventBus {
        // The event bus is always installed; unwrap is sound.
        let inner = self
            .instance_data::<builtin_plugins::event_bus::EventBusInner>()
            .expect("EventBus always installed by TurStdPlugin");
        builtin_plugins::event_bus::EventBus::from_inner(inner)
    }

    /// Combined focused-element state — single call replaces the
    /// `focused_is_editable()` + `focused_cursor_rect()` pair when the
    /// caller needs both. Phase 7's worker→main push will populate the
    /// main-side cache via `MainMsg::FocusedStateChanged`; today this
    /// reads live from the engine state (single-threaded).
    pub fn focused_state(&self) -> FocusedState {
        FocusedState {
            is_editable: self.focused_is_editable(),
            cursor_rect: self.focused_cursor_rect(),
        }
    }

    /// Push a platform (input) event from the embedder — resize, pointer,
    /// wheel, key, IME, or paste. These are dispatched to handlers via
    /// [`AppHandler::handle_platform_event`](core::handler::AppHandler::handle_platform_event).
    /// Also re-arms an idle autonomous loop (see [`Self::start`]) so the event
    /// is processed on the next frame.
    pub fn push_platform_event(&self, event: core::platform::PlatformEvent) {
        self.handle_worker_msg(WorkerMsg::PlatformEvent(event));
    }

    /// Push an engine-internal event onto the app-event bus (programmatic
    /// scrolls, clipboard writes). Most embedders only need
    /// [`Self::push_platform_event`] / [`Self::request_paint`]; this is
    /// exposed for host-initiated app events and testing. Re-arms an idle
    /// autonomous loop like [`Self::push_platform_event`].
    pub fn push_app_event(&self, event: core::app::AppEvent) {
        // AppEvent can't cross a thread boundary yet (its Custom payload's
        // Send bound lands in Phase 6). Single-threaded inline dispatch:
        // push straight onto the queue. Phase 7 will introduce a
        // `WorkerMsg::AppEvent` variant once AppEvent is Send.
        self.internal
            .app_context
            .borrow_mut()
            .app_event_queue
            .push(event);
        self.request_wakeup();
    }

    /// Request a paint on the next frame. Sets the `need_paint` flag directly
    /// (no event is enqueued), which the flush loop turns into a re-layout +
    /// re-render. Re-arms an idle autonomous loop so the request is processed
    /// even when nothing else is pending (see [`Self::start`]). Used by
    /// embedders after loading JS and by tests asserting an explicit paint.
    pub fn request_paint(&self) {
        self.handle_worker_msg(WorkerMsg::RequestPaint);
    }

    /// Internal dispatch — every [`WorkerMsg`] variant is processed here.
    ///
    /// **Phase 4** (single-threaded): the public API is a thin wrapper that
    /// builds a `WorkerMsg`, calls this method on the same thread, and
    /// unwraps the [`Reply`] synchronously.
    /// **Phase 7** (multi-threaded): a worker task replaces this method's
    /// body — `while let Some(msg) = rx.blocking_recv() { match msg { … } }`
    /// — and the public API sends over a real `mpsc`. The wire types stay
    /// the same.
    pub(crate) fn handle_worker_msg(&self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::PlatformEvent(event) => {
                self.internal
                    .app_context
                    .borrow_mut()
                    .platform_event_queue
                    .push(event);
                self.request_wakeup();
            }
            WorkerMsg::RequestPaint => {
                self.internal.js_context.need_paint.set(true);
                self.request_wakeup();
            }
            WorkerMsg::Wake => {
                // Single-threaded Phase 4: caller (pump) drives flush
                // directly so it can return FrameOutcome. No-op here;
                // Phase 7's worker task will run flush inline at this
                // case and emit MainMsg::FrameOutcome over the channel.
            }
            WorkerMsg::LoadModule { source, reply } => {
                let res = self.load_module_inner(&source);
                reply.send(res);
            }
            WorkerMsg::LoadJs { source, reply } => {
                let res = self.load_js_inner(&source);
                reply.send(res);
            }
            WorkerMsg::EvalModule { source, reply } => {
                let res = self.eval_module_inner(&source);
                reply.send(res);
            }
            WorkerMsg::DevElementTree { reply } => {
                let snap = self.dev_tool_element_tree();
                reply.send(snap);
            }
            WorkerMsg::DevGetElement { id, reply } => {
                let snap = self.dev_tool_get_element(id);
                reply.send(snap);
            }
            WorkerMsg::EventBusToJs(_bytes) => {
                // Phase 5 wires the event bus end-to-end. Placeholder:
                // log + drop until the JS-side sink is plumbed.
                tracing::trace!(
                    "EventBusToJs: {} bytes (Phase 5 wires delivery)",
                    _bytes.len()
                );
            }
            WorkerMsg::Destroy { reply } => {
                // Single-threaded Phase 4: lifetime is owned by the
                // embedder's `Rc<TurApp>`. Phase 7's worker task drains
                // and exits here.
                reply.send(());
            }
        }
    }

    fn load_js_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        boa.eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                ModuleError::Eval(e.to_string())
            })?;
        if let Err(e) = self.executor.drain(&mut boa) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
        Ok(())
    }

    fn load_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("entry.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("module parse error: {e}");
            ModuleError::Parse(e.to_string())
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("module run_jobs error: {e}");
        }
        drop(boa);
        if let Err(e) = self.executor.drain(&mut self.boa_context.borrow_mut()) {
            tracing::error!("load_module drain error: {e}");
        }
        Ok(())
    }

    fn eval_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("eval.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("eval_module parse error: {e}");
            ModuleError::Parse(e.to_string())
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("eval_module run_jobs error: {e}");
        }
        drop(boa);
        let _ = self.executor.drain(&mut self.boa_context.borrow_mut());
        Ok(())
    }

    /// Begin autonomous operation: the engine schedules its own frames via
    /// `driver`. The driver fires the engine's wake trampoline when due;
    /// each wake runs one [`Self::run_frame`], the [`Self::after_frame`] hook,
    /// then requests the next wake-up per the frame outcome. Input pushed
    /// via [`Self::push_platform_event`] re-arms an idle loop automatically.
    ///
    /// Must be called exactly once, after JS is loaded. The engine holds a
    /// `Weak` back-reference (no reference cycle), so the loop stops when the
    /// last `Rc<TurApp>` is dropped.
    pub fn start(self: &Rc<Self>, driver: Rc<dyn LoopDriver>) {
        let wake_fn: Rc<dyn Fn()> = {
            let weak = Rc::downgrade(self);
            Rc::new(move || {
                if let Some(app) = weak.upgrade() {
                    app.wake();
                }
            })
        };
        driver.set_wake(wake_fn.clone());
        *self.wake_fn.borrow_mut() = Some(wake_fn);
        *self.driver.borrow_mut() = Some(driver);
        // Kick off frame 1.
        self.wake();
    }

    /// Install a callback fired after each autonomous frame (in [`Self::wake`],
    /// after `run_frame`, before rescheduling). The wasm embedder uses it for
    /// DOM side-effects (file-pick resolution, textarea focus / caret
    /// positioning). Has no effect for manually-pumped (test) operation.
    pub fn set_after_frame_hook(&self, hook: Option<Rc<dyn Fn(FrameOutcome)>>) {
        *self.after_frame.borrow_mut() = hook;
    }

    /// Re-arm an idle autonomous loop: ask the driver for one wake-up on the
    /// next frame. No-op when no driver is installed (tests) or when a frame
    /// is already pending — the driver treats `request_next` as idempotent.
    fn request_wakeup(&self) {
        if let Some(driver) = self.driver.borrow().as_ref() {
            driver.request_next(core::app::NextFrame::Vsync);
        }
    }

    /// One autonomous-frame tick: `run_frame`, the `after_frame` hook, then
    /// reschedule via the driver. Called by the wake trampoline the driver
    /// fires (and by [`Self::start`] for the first frame).
    fn wake(self: &Rc<Self>) {
        let outcome = match self.run_frame() {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("frame loop run_frame error: {e}");
                return;
            }
        };
        if let Some(hook) = self.after_frame.borrow().as_ref() {
            hook.clone()(outcome);
        }
        let next = outcome.schedule;
        if let Some(driver) = self.driver.borrow().as_ref().cloned() {
            driver.request_next(next);
        }
    }

    pub fn dev_tool_element_tree(&self) -> Option<core::elements::DevNodeData> {
        let tree = self.internal.js_context.element_tree.borrow();
        let root_id = tree.root_element_id()?;
        tree.dev_tool_node(root_id.into())
    }

    pub fn dev_tool_get_element(
        &self,
        id: core::element::NodeId,
    ) -> Option<core::elements::DevNodeData> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .dev_tool_node(id)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .query_element(key)
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.js_context.focus_manager.borrow().focused()
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let tree = self.internal.js_context.element_tree.borrow();
        let node = tree.get_element(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.js_context.element_tree.borrow();

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

    /// True if the currently-focused element is an editable text element.
    /// Used by embedders (e.g. tur-wasm) to manage IME state.
    pub fn focused_is_editable(&self) -> bool {
        use core::focus::helper;
        let tree = self.internal.js_context.element_tree.borrow();
        let focus = self.internal.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, NodeTreeData> {
        self.internal.js_context.element_tree.borrow()
    }

    pub fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.internal.app_context.borrow_mut().render_to_pixels()
    }

    /// Override the shell's cursor backend. Used by embedders whose cursor
    /// target is per-instance (e.g. the wasm embedder's `WasmCursor` holds the
    /// instance's canvas DOM element) and thus can't be a shared runtime-level
    /// capability. Called after [`TurRuntime::create_app`] (which installs the
    /// runtime's `CursorCap` or falls back to `NoopCursor`).
    pub fn set_cursor_backend(
        &self,
        backend: Rc<std::cell::RefCell<dyn core::platform::CursorBackend>>,
    ) {
        self.internal
            .app_context
            .borrow_mut()
            .shell
            .set_cursor_platform(backend);
    }
}

/// Autonomous-loop driver — the platform scheduling primitive the engine
/// uses to wake itself for the next frame. Implementations live in the
/// embedder: a wasm driver backed by `requestAnimationFrame` / `setTimeout`
/// for the wake trampoline (tur-wasm), or any other platform's wake mechanism.
/// Tests do not install one (they pump [`TurApp::run_frame`] manually).
pub trait LoopDriver {
    /// Install the engine's wake trampoline. The driver must call it exactly
    /// once whenever a wake-up requested via [`Self::request_next`] becomes
    /// due. Set once at [`TurApp::start`].
    fn set_wake(&self, wake: Rc<dyn Fn()>);

    /// Request the next wake-up, replacing any pending request.
    /// - [`NextFrame::Vsync`](core::app::NextFrame) → wake on the next display
    ///   frame (~16 ms).
    /// - [`NextFrame::After(d)`](core::app::NextFrame) → wake after `d`.
    /// - [`NextFrame::Idle`](core::app::NextFrame) → cancel any pending
    ///   wake-up; the loop stops until [`TurApp::push_platform_event`] (via
    ///   `request_next(Vsync)`) re-arms it.
    fn request_next(&self, next: core::app::NextFrame);
}
