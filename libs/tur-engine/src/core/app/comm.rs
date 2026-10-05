//! Worker ↔ host message vocabulary.
//!
//! The engine runs on a worker thread (driven by the engine-internal
//! host-side backend in `core::runtime`); the embedder drives it from the
//! host thread via [`WorkerMsg`]s and receives
//! [`HostMsg`] replies. Every public `TurApp` method is a thin wrapper
//! that builds a `WorkerMsg`, sends it via the channel, and awaits the
//! matching [`Reply`] (one-shot slot).
//!
//! ## Channel topology
//!
//! All channels use [`futures::channel::mpsc`] (multi-producer-single-consumer
//! unbounded) and [`futures::channel::oneshot`] (single-shot):
//!
//! | Channel       | Direction       | Capacity    | Sender side            | Receiver side           |
//! |---------------|-----------------|-------------|------------------------|-------------------------|
//! | `WorkerMsg`   | host → worker   | unbounded   | main `unbounded_send`  | worker `next().await`   |
//! | `HostMsg`     | worker → host   | unbounded   | worker `unbounded_send`| main `next().await`     |
//! | `Reply<T>`    | worker → host   | oneshot     | worker `send` (consume)| main `await`            |
//!
//! ## Why `futures::channel` over `async_channel`
//!
//! `async_channel` internally uses `event_listener`, which on contention takes
//! a `std::sync::Mutex`. On the wasm32 main thread that mutex's `lock_contended`
//! calls `Atomics.wait` — forbidden by the JS spec on the main thread, so it traps
//! with "RuntimeError: Atomics.wait cannot be called in this context".
//! `futures::channel` uses Waker-based notification (no futex, no
//! `event_listener`), so it is safe to poll on the wasm main thread.
//!
//! ## Send-ness
//!
//! Every variant is `Send` (verified by the compile-time assertion at the
//! bottom of this file).

use std::fmt;
use std::sync::Arc;

use crate::core::app::FrameOutcome;
use crate::core::elements::NodeTreeData;
use crate::core::focus::FocusManager;
use crate::core::platform::PlatformEvent;
use crate::core::render::RenderCommand;
use crate::core::shell::{Cursor, TextInputState};

/// Type-erased closure run against the worker's live tree + focus state
/// (see [`WorkerMsg::WithTree`]). The tree always exists (instance-owned);
/// it may be root-less before the first `mount` / after teardown. Ships its
/// result via a `Reply` channel it captures, so `WorkerMsg` stays
/// monomorphic.
pub type TreeRunner = Box<dyn FnOnce(&NodeTreeData, &FocusManager) + Send + 'static>;

/// Which dev-tool snapshot the worker should serialize (see `core::dev`).
#[derive(Debug, Clone, Copy)]
pub enum DevToolRequest {
    /// The root node's JSON snapshot.
    ElementTree,
    /// One node's JSON snapshot by raw id.
    GetElement(u64),
    /// The per-instance frame-stats JSON snapshot.
    FrameStats,
}

/// host → worker channel sender. Unbounded — the host side pushes input
/// (platform events, wake, RPC requests) and the worker drains them in
/// arrival order.
pub type WorkerTx = futures::channel::mpsc::UnboundedSender<WorkerMsg>;
/// host → worker channel receiver. Held by the worker thread; awaited in
/// `worker_loop`.
pub type WorkerRx = futures::channel::mpsc::UnboundedReceiver<WorkerMsg>;

/// worker → host channel sender. Unbounded — the worker ships per-frame
/// messages (render batch, FrameOutcome, cursor / focus changes) without
/// coordinating with the host side. The host side drains them in the run loop's recv loop.
pub type HostTx = futures::channel::mpsc::UnboundedSender<HostMsg>;
/// worker → host channel receiver. Held by the host thread; drained by
/// [`TurAppLooper::run`](crate::TurAppLooper::run).
pub type HostRx = futures::channel::mpsc::UnboundedReceiver<HostMsg>;

/// host → worker. All input that can drive the engine flows through one of
/// these variants.
pub enum WorkerMsg {
    /// DOM / JNI / winit platform event (pointer, key, wheel, IME, resize,
    /// …). Dispatched to subsystems via `handle_platform_event` on the next
    /// flush iteration.
    PlatformEvent(PlatformEvent),
    /// Drive one flush iteration. Sent by main's rAF loop. The worker then
    /// emits [`HostMsg::RenderCommands`] (if it painted) and
    /// [`HostMsg::FrameOutcome`].
    Wake,
    /// Parse + compile + boot a **rut** module and invoke its `entry fn
    /// start()` (the module lifecycle contract: `entry fn stop()` — when
    /// present — runs before the next load and at destroy). Reply carries
    /// the parse/boot outcome. `Arc<str>` because module sources can be
    /// large (the playground ships multi-KB bundles) — `Arc` lets the
    /// message be duplicated cheaply if needed.
    LoadRutModule {
        source: Arc<str>,
        reply: ReplySender<Result<(), ModuleError>>,
    },
    /// Call a named `entry fn(u64, f64)` on the loaded rut module — the
    /// engine→rut event rail (input dispatch, embedder events). A missing
    /// entry is a successful no-op (event rails are optional).
    CallRutEntry {
        name: Arc<str>,
        a: u64,
        b: f64,
        reply: ReplySender<Result<(), ModuleError>>,
    },
    /// Read the loaded rut module's `entry fn start() -> u64` answer
    /// (0 when `start` returns nil or no rut module is loaded).
    RutStartAnswer {
        reply: ReplySender<u64>,
    },
    /// Dev-tool snapshot request (the E3 rut-row surface): the worker
    /// serializes the requested view of its live state to a JSON string
    /// (see `core::dev`).
    DevTool {
        req: DevToolRequest,
        reply: ReplySender<String>,
    },
    /// Frame-timing toggle (the host-side `setHostFrameTiming`): the worker
    /// mirrors its `FrameStats.host_timing_enabled` flag so
    /// `frameStats()` reports it and the flush records per-frame timings.
    FrameTimingEnabled { enabled: bool },
    /// Test-only: run a closure against the worker's live `NodeTreeData`
    /// AND `FocusManager` — everything needed to reconstruct the former
    /// per-field focus/dev-tool queries (`focused_cursor_rect`,
    /// `focused_is_editable`, `focused_element`, `query_element`,
    /// `dev_tool_get_element`, ...) on the caller side: the closure runs
    /// on the worker thread (where the tree + its boxed `AnyElement`s
    /// live), so it can do typed introspection that isn't serializable
    /// across the thread boundary (e.g. `element.cast::<TextElement>()
    /// .spans()`). The closure ships its result via a reply channel it
    /// captures, so the enum stays monomorphic.
    WithTree { runner: TreeRunner },
    /// Push an engine-internal event (programmatic scroll, clipboard
    /// write, etc.).
    AppEvent(crate::core::app::AppEvent),
    /// Clear the instance's focus. Sent by a hosting parent through the
    /// virtual-app control rail ([`VirtualControl::ClearFocus`] — a
    /// pointer click elsewhere in the parent took focus away from the
    /// hosted child): keys are focus-routed, so a blurred child must
    /// release its focused element or it would keep consuming (invisible)
    /// keystrokes. Drives a flush like [`WorkerMsg::Wake`].
    BlurFocus,
    /// Host-registered image receipt (`TurApp::register_image`): the host
    /// minted the id (host range — see
    /// [`HOST_IMAGE_ID_BASE`](crate::core::image_resource::HOST_IMAGE_ID_BASE)),
    /// retained the pixel Blob host-side, uploaded it to its renderer, and
    /// ships the worker only the natural size so layout + paint can serve
    /// the id. Fire-and-forget with no Reply: the host side is already
    /// committed, and the shared FIFO worker channel guarantees this is
    /// processed before any later message that could hand the id to module
    /// code.`
    RegisterImageMetadata {
        id: crate::core::image_resource::ImageResourceId,
        size: crate::core::layout::Size,
    },
    /// Initiate shutdown. Worker drains pending work, replies when safe
    /// to drop.
    Destroy { reply: ReplySender<()> },
    /// Host-side render-commit timings, pushed back per painted frame when
    /// frame timing is enabled (`turDevTool.setHostFrameTiming(true)`).
    /// Fire-and-forget (no wake — a busy worker delivers it within the
    /// current frame stream; an idle worker doesn't need it). Recorded into
    /// the instance's `FrameStats::last_host`.
    FrameTiming {
        /// The flush epoch of the frame these timings belong to (echoed
        /// from the `RenderCommands` stamp).
        frame_id: u64,
        /// Scene rebuild + command playback (`Renderer::render_commands`).
        apply_us: u64,
        /// Encode + raster + composite (`Renderer::present`).
        present_us: u64,
    },
}

/// worker → host. Emitted by the worker either during a flush
/// ([`HostMsg::RenderCommands`], [`HostMsg::Shell`]) or in response to a
/// [`WorkerMsg`] RPC (`Reply<T>` slots).
pub enum HostMsg {
    /// One frame's worth of paint state. Main applies the batch to its
    /// renderer (owned by `HostBackend`) directly, at the render commit
    /// point.
    ///
    /// Images are NOT shipped here — they travel once per new resource via
    /// [`HostMsg::UploadImage`] (main uploads them into its atlas
    /// incrementally). Geometry travels WITH the batch: `viewport` is what
    /// this frame was laid out for (the worker's `Screen` at record time),
    /// and the host syncs its renderer to it immediately before playback —
    /// so the backing-store swap and the frame content are one atomic
    /// operation (never a cleared surface between a resize and its
    /// replacement frame).
    RenderCommands {
        commands: Vec<RenderCommand>,
        viewport: crate::core::screen::ScreenViewport,
        /// The flush epoch the batch was recorded under (stamped by the
        /// worker). Echoed back in `WorkerMsg::FrameTiming` so host-side
        /// render-commit timings can be attributed to the right frame.
        frame_id: u64,
    },
    /// A newly-registered image resource (`createImageResource` /
    /// `createSvgResource` on the worker). Shipped exactly once per id
    /// (sent directly from the `createImageResource` bridge via the shared
    /// `host_tx`); main uploads it to the renderer's image atlas and
    /// retains the `ImageResource` (pixel `Blob`) keyed by
    /// `ImageResourceId` for context-loss re-upload.
    UploadImage {
        id: crate::core::image_resource::ImageResourceId,
        image: crate::core::image_resource::ImageResource,
    },
    /// Schedule decision after a flush. Main arms the next rAF /
    /// `setTimeout` based on `schedule`. The `Err(String)` variant
    /// carries a flush error message (worker can't ship `TurError`
    /// directly because its `JsEval` variant holds a boa `JsError` which
    /// is `!Send` — main re-wraps as `TurError::Other`).
    FrameOutcome(Result<FrameOutcome, String>),
    /// A shell-layer request (cursor / text-input) changed this frame —
    /// deduped per command kind, shipped only on change. Main applies it
    /// to the embedder-supplied [`Shell`](crate::core::shell::Shell)
    /// inside `apply_msg`.
    Shell(ShellCommand),
    /// Virtual-app control (spawn / resize / platform-event / destroy a
    /// hosted child instance) — routed by `TurAppLooper` (the drain point)
    /// straight to the instance's
    /// [`VirtualHost`](crate::core::virtual_app::VirtualHost). The child
    /// lifecycle surface lives in `core::virtual_app`; status + frame egress
    /// flows back through `WorkerMsg::AppEvent(AppEvent::custom(...))`, so
    /// this is the only virtual-app message variant.
    VirtualControl(crate::core::virtual_app::VirtualControl),
    /// A module runtime error no caller could observe (a trapping entry /
    /// view closure / derive / async task, or fuel exhaustion). Routed by
    /// `TurAppLooper` to the
    /// instance's [`VirtualHost`](crate::core::virtual_app::VirtualHost):
    /// an element-hosted child forwards it to its parent's worker
    /// (`VirtualErrorEvent` → the controller's `onRuntimeError$`); the
    /// embedder-hosted root logs it.
    RuntimeError {
        report: crate::core::app::runtime_error::RuntimeErrorReport,
    },
    /// The instance's focus changed (its own `FocusManager` gained or
    /// cleared a focused element). Routed by `TurAppLooper` to the
    /// instance's [`VirtualHost`](crate::core::virtual_app::VirtualHost):
    /// an element-hosted child forwards it to its parent's worker
    /// (`VirtualFocusEvent` — the parent's focus manager learns "focus
    /// sits inside this host", which is what makes key routing work); the
    /// embedder-hosted root has no parent — dropped there.
    FocusChanged { focused: bool },
    /// Worker finished shutting down (response to `WorkerMsg::Destroy`).
    Destroyed,
    /// Enable/disable host-side frame-timing collection (the per-frame
    /// `WorkerMsg::FrameTiming` push-back). Sent once per toggle from the
    /// `turDevTool.setHostFrameTiming(...)` bridge; `HostBackend` applies it
    /// to its local gate.
    FrameTimingEnabled(bool),
    /// A dev-tool snapshot reply, shipped worker → host so the JSON is
    /// resolved on the MAIN thread (wasm only — see `worker_loop`'s
    /// intercept). A oneshot waker fired on the worker thread can never
    /// re-poll a main-thread `wasm_bindgen_futures` task (thread-local
    /// task queues), so the reply must ride this drained channel:
    /// `HostBackend::apply_msg` runs on main, and firing the oneshot
    /// there wakes the awaiting task on the thread that spawned it.
    DevToolReply {
        reply: ReplySender<String>,
        json: String,
    },
}

/// A deduped shell-layer request shipped worker → host inside
/// [`HostMsg::Shell`]. The worker dedups each kind independently against
/// the last emitted value (cursor / text-input have separate caches), so
/// each variant arrives only on change.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellCommand {
    /// The resolved pointer shape changed (deepest painted `MouseRegion`
    /// claim). Applied via [`Shell::set_cursor`](crate::core::shell::Shell::set_cursor).
    SetCursor(Cursor),
    /// The focused element's text-input session state changed (IME active
    /// flag + caret rect). Applied via
    /// [`Shell::request_text_input`](crate::core::shell::Shell::request_text_input).
    RequestTextInput(TextInputState),
}

/// Error returned from module load / entry-call RPCs.
#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    /// Parse failure (syntax / compile diagnostics).
    #[error("module parse error: {0}")]
    Parse(String),
    /// Boot / evaluation failure (a trapping `start`, a missing entry, …).
    #[error("module evaluation error: {0}")]
    Eval(String),
    /// Worker task dropped before replying.
    #[error("worker gone")]
    WorkerGone,
}

/// One-shot reply slot — sender side. Wraps a
/// `futures::channel::oneshot::Sender<T>`. The sender fires once (via
/// `send`, which consumes it); the receiver awaits the value via
/// `rx.await`.
pub struct ReplySender<T> {
    pub(crate) tx: futures::channel::oneshot::Sender<T>,
}

/// One-shot reply slot — receiver side. `rx.await` yields the value once
/// the sender fires. Held by main; the worker ships the reply through the
/// sender half.
pub struct Reply<T> {
    pub(crate) rx: futures::channel::oneshot::Receiver<T>,
}

impl<T> Reply<T> {
    /// Create a paired (sender, receiver) slot pair backed by
    /// `futures::channel::oneshot::channel`.
    pub fn pair() -> (ReplySender<T>, Reply<T>) {
        let (tx, rx) = futures::channel::oneshot::channel();
        (ReplySender { tx }, Reply { rx })
    }
}

impl<T> ReplySender<T> {
    /// Fire the reply. Consumes the sender (one-shot semantics). The
    /// receiver is always awaiting at fire time (RPC replies are
    /// request/response), so `send` succeeds unless main dropped the
    /// receiver first — in which case the value is dropped silently.
    pub fn send(self, value: T) {
        let _ = self.tx.send(value);
    }
}

// Manual Debug impls — `PlatformEvent` / `DevNodeData` don't derive Debug,
// and the reply slots shouldn't print their payload.
impl fmt::Debug for WorkerMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlatformEvent(_) => f.debug_tuple("PlatformEvent").finish_non_exhaustive(),
            Self::Wake => write!(f, "Wake"),
            Self::LoadRutModule { source, .. } => f
                .debug_struct("LoadRutModule")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::CallRutEntry { name, a, b, .. } => f
                .debug_struct("CallRutEntry")
                .field("name", &name.as_ref())
                .field("a", a)
                .field("b", b)
                .finish_non_exhaustive(),
            Self::RutStartAnswer { .. } => f.write_str("RutStartAnswer"),
            Self::DevTool { req, .. } => f.debug_struct("DevTool").field("req", req).finish(),
            Self::FrameTimingEnabled { enabled } => f
                .debug_struct("FrameTimingEnabled")
                .field("enabled", enabled)
                .finish(),
            Self::WithTree { .. } => f.debug_struct("WithTree").finish(),
            Self::AppEvent(_) => f.debug_tuple("AppEvent").finish_non_exhaustive(),
            Self::BlurFocus => f.write_str("BlurFocus"),
            Self::RegisterImageMetadata { id, .. } => {
                f.debug_tuple("RegisterImageMetadata").field(id).finish()
            }
            Self::Destroy { .. } => write!(f, "Destroy"),
            Self::FrameTiming {
                frame_id,
                apply_us,
                present_us,
            } => f
                .debug_struct("FrameTiming")
                .field("frame_id", frame_id)
                .field("apply_us", apply_us)
                .field("present_us", present_us)
                .finish(),
        }
    }
}

impl fmt::Debug for HostMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RenderCommands { commands, .. } => f
                .debug_tuple("RenderCommands")
                .field(&commands.len())
                .finish(),
            Self::UploadImage { id, .. } => f.debug_tuple("UploadImage").field(id).finish(),
            Self::FrameOutcome(fo) => f.debug_tuple("FrameOutcome").field(fo).finish(),
            Self::Shell(cmd) => f.debug_tuple("Shell").field(cmd).finish(),
            Self::VirtualControl(c) => f.debug_tuple("VirtualControl").field(c).finish(),
            Self::RuntimeError { report } => f.debug_tuple("RuntimeError").field(report).finish(),
            Self::FocusChanged { focused } => f
                .debug_struct("FocusChanged")
                .field("focused", focused)
                .finish(),
            Self::Destroyed => write!(f, "Destroyed"),
            Self::FrameTimingEnabled(on) => f.debug_tuple("FrameTimingEnabled").field(on).finish(),
            Self::DevToolReply { .. } => f.write_str("DevToolReply"),
        }
    }
}

impl<T> fmt::Debug for Reply<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reply").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for ReplySender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplySender").finish_non_exhaustive()
    }
}

// Compile-time Send assertions — guard against future variants breaking
// the worker↔host channel contract. If these fail, a new field's type
// isn't Send and needs wrapping (typically `Arc<T>`).
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<WorkerMsg>();
    assert_send::<HostMsg>();
    assert_send::<ModuleError>();
    // Channels themselves must be Send + Sync.
    assert_send::<WorkerTx>();
    assert_send::<WorkerRx>();
    assert_send::<HostTx>();
    assert_send::<HostRx>();
};

#[cfg(test)]
mod tests {
    use super::*;

    /// `Arc<str>` is the canonical source carrier — verify it round-trips
    /// through `WorkerMsg::LoadRutModule` without clone friction.
    #[test]
    fn load_module_carries_arc_str() {
        let (tx, _rx) = Reply::<Result<(), ModuleError>>::pair();
        let source: Arc<str> = Arc::from("entry fn start() {}");
        let msg = WorkerMsg::LoadRutModule { source, reply: tx };
        assert!(matches!(msg, WorkerMsg::LoadRutModule { .. }));
    }

    /// `ModuleError` Display strings are stable (used for diagnostics).
    #[test]
    fn module_error_display() {
        assert_eq!(
            ModuleError::Parse("syn".into()).to_string(),
            "module parse error: syn"
        );
        assert_eq!(
            ModuleError::Eval("run".into()).to_string(),
            "module evaluation error: run"
        );
        assert_eq!(ModuleError::WorkerGone.to_string(), "worker gone");
    }

    /// Reply slot pair — sender fires, receiver drains (oneshot's Receiver
    /// is itself a Future — no synchronous `try_recv` exists, so we drive
    /// it via `block_on`).
    #[test]
    fn reply_slot_round_trip() {
        let (_tx, rx) = Reply::<u32>::pair();
        // Pending state isn't easily observable on oneshot without
        // polling; the round-trip below covers the success path.
        let _ = rx;

        let (tx2, rx2) = Reply::<u32>::pair();
        tx2.send(42);
        let val = futures::executor::block_on(rx2.rx).unwrap();
        assert_eq!(val, 42);
    }

    /// A dropped sender (without firing) leaves the receiver empty —
    /// `oneshot::Receiver::await` resolves to `Err(Canceled)`.
    #[test]
    fn reply_slot_dropped_sender_leaves_none() {
        let (tx, rx) = Reply::<u32>::pair();
        drop(tx);
        let result = futures::executor::block_on(rx.rx);
        assert!(result.is_err());
    }
}
