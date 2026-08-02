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
pub use crate::core::event_bus::EventBus;
// Re-export the runtime + builder at the crate root — the primary entry point
// for embedders. `TurRuntime::builder()` is the shared, created-once object;
// `runtime.create_app()` / `runtime.create_headless_app()` spawn isolated
// `TurApp` instances.
pub use crate::core::runtime::{InlineBackend, TurRuntime, TurRuntimeBuilder};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use boa_engine::Context;

use error::TurError;

use core::app::{FrameOutcome, ModuleError, Reply, WorkerMsg};
use core::element::{ElementNodeId, NodeId};
use core::elements::AnyElement;
use core::runtime::backend::{BoaClosure, ElementClosure, TurAppBackend};

#[cfg(feature = "trace")]
use core::elements::NodeTreeData;

pub struct TurApp {
    /// Engine state + dispatch — owned by the backend. `InlineBackend`
    /// runs everything on this thread (today's behavior, used by tests);
    /// `ThreadedBackend` (Phase 7 follow-up) runs the engine on a worker.
    backend: Box<dyn TurAppBackend>,
    /// Autonomous-loop driver. `None` until [`Self::start`] is called
    /// (production); tests leave it unset and pump via [`Self::run_frame`].
    /// Always main-side: even in threaded mode, the driver's wake
    /// trampoline must fire `TurApp::wake` on the main thread.
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
    /// Construct a `TurApp` backed by the given [`TurAppBackend`]. The
    /// runtime calls this with [`InlineBackend`] for `create_app` /
    /// `create_headless_app`; Phase 7's `create_app_threaded` will pass
    /// a `ThreadedBackend`.
    pub fn new(backend: Box<dyn TurAppBackend>) -> Self {
        Self {
            backend,
            driver: RefCell::new(None),
            wake_fn: RefCell::new(None),
            after_frame: RefCell::new(None),
        }
    }

    pub fn load_js(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        self.backend.handle_worker_msg(WorkerMsg::LoadJs {
            source: Arc::from(source),
            reply: tx,
        });
        rx.recv().map_err(TurError::from)
    }

    pub fn load_module(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_module: evaluating module ({} bytes)", source.len());
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        self.backend.handle_worker_msg(WorkerMsg::LoadModule {
            source: Arc::from(source),
            reply: tx,
        });
        rx.recv().map_err(TurError::from)
    }

    pub fn eval_module(&self, source: &str) -> Result<(), TurError> {
        let (tx, rx) = Reply::<Result<(), ModuleError>>::pair();
        self.backend.handle_worker_msg(WorkerMsg::EvalModule {
            source: Arc::from(source),
            reply: tx,
        });
        rx.recv().map_err(TurError::from)
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
        self.backend.pump()
    }

    /// Legacy alias for [`Self::pump`]. Kept for embedder/test back-compat
    /// during the Phase 4 transition; new code should call `pump`.
    pub fn run_frame(&self) -> Result<core::app::FrameOutcome, TurError> {
        self.pump()
    }

    /// Escape hatch — run a closure with the boa `Context`. **Inline-only**:
    /// panics on the threaded backend (it doesn't ship arbitrary closures
    /// across threads — production threaded code uses specific RPC variants
    /// like `load_module` / `eval_module`).
    pub fn with_boa_context<R: 'static>(&self, f: impl FnOnce(&mut Context) -> R + 'static) -> R {
        let boxed: BoaClosure = Box::new(move |ctx| Box::new(f(ctx)));
        let result = self.backend.with_boa_context_dyn(boxed);
        *result
            .downcast::<R>()
            .expect("with_boa_context: backend returned wrong type")
    }

    /// Always-installed event bus handle. The bus is unconditionally
    /// created by `TurAppInternal::new` and wired up by
    /// `TurStdPlugin::register` via `install_event_bus`, so this never
    /// fails — the historical `EventBus::of(&app) -> Option<EventBus>`
    /// was always `Some`. New code should prefer this direct accessor.
    ///
    /// `EventBus::of` is kept as a back-compat alias (returns
    /// `Some(self.event_bus())`).
    pub fn event_bus(&self) -> Rc<core::event_bus::EventBus> {
        self.backend.event_bus()
    }

    /// Combined focused-element state — single call replaces the
    /// `focused_is_editable()` + `focused_cursor_rect()` pair when the
    /// caller needs both. Phase 7's worker→main push will populate the
    /// main-side cache via `MainMsg::FocusedStateChanged`; today this
    /// reads live from the engine state (single-threaded).
    pub fn focused_state(&self) -> FocusedState {
        self.backend.focused_state()
    }

    /// Push a platform (input) event from the embedder — resize, pointer,
    /// wheel, key, IME, or paste. These are dispatched to handlers via
    /// [`AppHandler::handle_platform_event`](core::handler::AppHandler::handle_platform_event).
    /// Also re-arms an idle autonomous loop (see [`Self::start`]) so the event
    /// is processed on the next frame.
    pub fn push_platform_event(&self, event: core::platform::PlatformEvent) {
        self.backend
            .handle_worker_msg(WorkerMsg::PlatformEvent(event));
        self.request_wakeup();
    }

    /// Push an engine-internal event onto the app-event bus (programmatic
    /// scrolls, clipboard writes). Most embedders only need
    /// [`Self::push_platform_event`] / [`Self::request_paint`]; this is
    /// exposed for host-initiated app events and testing. Re-arms an idle
    /// autonomous loop like [`Self::push_platform_event`].
    pub fn push_app_event(&self, event: core::app::AppEvent) {
        // Inline backend writes directly to the queue. Threaded backend
        // needs `AppEvent` to be `Send` (its `Custom` payload's bound
        // lands with Phase 7's threaded work) — until then, threaded
        // panics here.
        self.backend.push_app_event(event);
        self.request_wakeup();
    }

    /// Request a paint on the next frame. Sets the `need_paint` flag directly
    /// (no event is enqueued), which the flush loop turns into a re-layout +
    /// re-render. Re-arms an idle autonomous loop so the request is processed
    /// even when nothing else is pending (see [`Self::start`]). Used by
    /// embedders after loading JS and by tests asserting an explicit paint.
    pub fn request_paint(&self) {
        self.backend.handle_worker_msg(WorkerMsg::RequestPaint);
        self.request_wakeup();
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
        self.backend.dev_tool_element_tree()
    }

    pub fn dev_tool_get_element(
        &self,
        id: core::element::NodeId,
    ) -> Option<core::elements::DevNodeData> {
        self.backend.dev_tool_get_element(id)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.backend.query_element(key)
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.backend.focused_element()
    }

    /// Escape hatch — run a closure with an element by id. Returns `None`
    /// if the element doesn't exist. **Inline-only** (panics on threaded
    /// backend — see [`Self::with_boa_context`]).
    pub fn with_element<R: 'static>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R + 'static,
    ) -> Option<R> {
        let boxed: ElementClosure = Box::new(move |e| Box::new(cb(e)));
        let result = self.backend.with_element_dyn(id, boxed)?;
        Some(
            *result
                .downcast::<R>()
                .expect("with_element: backend returned wrong type"),
        )
    }

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.backend.focused_cursor_rect()
    }

    /// True if the currently-focused element is an editable text element.
    /// Used by embedders (e.g. tur-wasm) to manage IME state.
    pub fn focused_is_editable(&self) -> bool {
        self.backend.focused_is_editable()
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, NodeTreeData> {
        self.backend.element_tree_handle()
    }

    pub fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.backend.render_to_pixels()
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
        self.backend.set_cursor_backend(backend);
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
