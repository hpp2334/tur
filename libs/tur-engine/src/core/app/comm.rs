//! Worker ↔ main message vocabulary.
//!
//! The engine runs on a worker thread (see [`crate::core::runtime::MainBackend`]);
//! the embedder drives it from main via [`WorkerMsg`]s and receives
//! [`MainMsg`] replies. Every public `TurApp` method is a thin wrapper
//! that builds a `WorkerMsg`, sends it via the channel, and awaits the
//! matching [`Reply`] (one-shot slot).
//!
//! ## Channel topology
//!
//! All three channels use [`async_channel`]:
//!
//! | Channel       | Direction       | Capacity    | Sender side       | Receiver side           |
//! |---------------|-----------------|-------------|-------------------|-------------------------|
//! | `WorkerMsg`   | main → worker   | unbounded   | main `try_send`   | worker `recv().await`   |
//! | `MainMsg`     | worker → main   | unbounded   | worker `try_send` | main `recv().await`     |
//! | `Reply<T>`    | worker → main   | bounded(1)  | worker `try_send` | main `recv().await`     |
//!
//! The worker thread itself runs a sync thread entry-point that wraps an
//! `async fn worker_loop(...)` via `futures::executor::block_on`. This lets
//! the worker `await` on `worker_rx.recv()` instead of blocking on a
//! `Mutex`+`Condvar` — the same primitive the main thread uses, so there's
//! a single concurrency model across the engine.
//!
//! ## Send-ness
//!
//! Every variant is `Send` (verified by the compile-time assertion at the
//! bottom of this file).

use std::fmt;
use std::sync::Arc;

use crate::core::app::FrameOutcome;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::DevNodeData;
use crate::core::image_resource::ImageResourceMap;
use crate::core::platform::Cursor;
use crate::core::platform::PlatformEvent;
use crate::core::render::RenderCommand;

/// main → worker channel sender. Unbounded — main pushes input (platform
/// events, wake, RPC requests) and the worker drains them in arrival order.
pub type WorkerTx = async_channel::Sender<WorkerMsg>;
/// main → worker channel receiver. Held by the worker thread; awaited in
/// `worker_loop`.
pub type WorkerRx = async_channel::Receiver<WorkerMsg>;

/// worker → main channel sender. Unbounded — the worker ships per-frame
/// messages (render batch, FrameOutcome, cursor / focus changes) without
/// coordinating with main. Main drains them in `pump`'s recv loop.
pub type MainTx = async_channel::Sender<MainMsg>;
/// worker → main channel receiver. Held by the main thread; awaited in
/// `MainBackend::pump`.
pub type MainRx = async_channel::Receiver<MainMsg>;

/// main → worker. All input that can drive the engine flows through one of
/// these variants.
pub enum WorkerMsg {
    /// DOM / JNI / winit platform event (pointer, key, wheel, IME, resize,
    /// …). Dispatched to subsystems via `handle_platform_event` on the next
    /// flush iteration.
    PlatformEvent(PlatformEvent),
    /// Mark the next frame for paint without enqueuing an event. Mirrors
    /// today's `TurApp::request_paint` — sets the `need_paint` flag.
    RequestPaint,
    /// Drive one flush iteration. Sent by main's rAF loop. The worker then
    /// emits [`MainMsg::RenderCommands`] (if it painted) and
    /// [`MainMsg::FrameOutcome`].
    Wake,
    /// Parse + load + evaluate a JS module. Reply carries the parse/eval
    /// outcome. `Arc<str>` because module sources can be large (the
    /// playground ships multi-KB compiled JS) — `Arc` lets the message be
    /// duplicated cheaply if needed (e.g. dev-tool logging).
    LoadModule {
        source: Arc<str>,
        reply: ReplySender<Result<(), ModuleError>>,
    },
    /// Evaluate a plain JS script (not a module).
    LoadJs {
        source: Arc<str>,
        reply: ReplySender<Result<(), ModuleError>>,
    },
    /// Parse + evaluate a JS module without `load_link_evaluate` semantics
    /// (used by `eval_module` for re-evaluation scenarios).
    EvalModule {
        source: Arc<str>,
        reply: ReplySender<Result<(), ModuleError>>,
    },
    /// Synchronous JS expression evaluation (test-only). Runs `ctx.eval(source)`
    /// on the worker, converts the result to its display string, and replies.
    /// Production code uses `LoadModule` / `EvalModule`; this is for tests
    /// that read JS-side state via `globalThis.__x = ...`.
    EvalJs {
        source: Arc<str>,
        reply: ReplySender<String>,
    },
    /// Dev-tool: full element-tree snapshot. RPC.
    DevElementTree {
        reply: ReplySender<Option<DevNodeData>>,
    },
    /// Dev-tool: single-node snapshot.
    DevGetElement {
        id: NodeId,
        reply: ReplySender<Option<DevNodeData>>,
    },
    /// Query the focused-element state.
    QueryFocusedState {
        reply: ReplySender<crate::FocusedState>,
    },
    /// Query the focused-element id.
    QueryFocusedElement {
        reply: ReplySender<Option<ElementNodeId>>,
    },
    /// Query the focused element's caret rect (logical-space `(x, y, w, h)`).
    QueryFocusedCursorRect {
        reply: ReplySender<Option<(f64, f64, f64, f64)>>,
    },
    /// Query whether the focused element is an editable text element.
    QueryFocusedIsEditable { reply: ReplySender<bool> },
    /// Path-based element lookup. `key` is the path segments.
    QueryElement {
        key: Vec<String>,
        reply: ReplySender<Option<NodeId>>,
    },
    /// Event bus — host → JS bytes. Worker forwards to the JS-side
    /// `__turEventBus.toJs` sink during the next flush.
    EventBusToJs(Vec<u8>),
    /// Push an engine-internal event (programmatic scroll, clipboard
    /// write, etc.).
    AppEvent(crate::core::app::AppEvent),
    /// Screenshot RPC — read rendered pixels from the worker's renderer.
    /// Used by screenshot tests; returns `None` if the renderer doesn't
    /// support pixel readback.
    RenderToPixels { reply: ReplySender<Option<Vec<u8>>> },
    /// Initiate shutdown. Worker drains pending work, replies when safe
    /// to drop.
    Destroy { reply: ReplySender<()> },
}

/// worker → main. Emitted by the worker either during a flush
/// ([`MainMsg::RenderCommands`], [`MainMsg::FocusedStateChanged`]) or in
/// response to a [`WorkerMsg`] RPC ([`MainMsg::DevReply`]).
pub enum MainMsg {
    /// One frame's worth of paint state. Main applies the batch to its
    /// renderer via the `render_sink` callback.
    ///
    /// `image_map` is the worker's full image resource map (Arc-cloned —
    /// cheap, since `ImageResource` wraps an Arc-backed `Blob`). Shipped
    /// every frame so the main-side renderer can upload any newly-added
    /// images. Optimization to ship only diffs is deferred.
    ///
    /// `viewport` is `(logical_width, logical_height, dpr)` — main calls
    /// `renderer.resize(...)` when this changes before applying the batch.
    RenderCommands {
        commands: Vec<RenderCommand>,
        image_map: Arc<ImageResourceMap>,
        viewport: (u32, u32, f64),
    },
    /// Schedule decision after a flush. Main arms the next rAF /
    /// `setTimeout` based on `schedule`. The `Err(String)` variant
    /// carries a flush error message (worker can't ship `TurError`
    /// directly because its `JsEval` variant holds a boa `JsError` which
    /// is `!Send` — main re-wraps as `TurError::Other`).
    FrameOutcome(Result<FrameOutcome, String>),
    /// Resolved cursor changed this frame (deduped: only emitted on
    /// change). Main forwards to its `CursorBackend` and caches it.
    CursorChanged(Cursor),
    /// Focused-element state changed (used by main for IME / caret
    /// placement on platforms where the IME target lives off-engine).
    /// Pushed once per change, not per frame. Main caches it for
    /// non-blocking reads from embedder callbacks.
    FocusedStateChanged {
        is_editable: bool,
        cursor_rect: Option<(f64, f64, f64, f64)>,
    },
    /// Event bus — JS → host bytes. One `MainMsg` per `__turEventBus.toHost`
    /// dispatch.
    EventBusToHost(Vec<u8>),
    /// Reply to a dev-tool RPC.
    DevReply(DevReply),
    /// Worker finished shutting down (response to `WorkerMsg::Destroy`).
    Destroyed,
}

/// Dev-tool RPC reply payload.
pub enum DevReply {
    ElementTree(Option<DevNodeData>),
    GetElement(Option<DevNodeData>),
}

/// Error returned from module load / eval RPCs.
#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    /// JS parse failure (syntax error, etc.).
    #[error("JS parse error: {0}")]
    Parse(String),
    /// JS evaluation failure (thrown error, etc.).
    #[error("JS evaluation error: {0}")]
    Eval(String),
    /// Worker task dropped before replying.
    #[error("worker gone")]
    WorkerGone,
}

/// One-shot reply slot — sender side. Wraps an `async_channel::Sender<T>`
/// with capacity 1. The sender fires once (via `try_send`); the receiver
/// awaits the value via `recv().await`.
pub struct ReplySender<T> {
    pub(crate) tx: async_channel::Sender<T>,
}

/// One-shot reply slot — receiver side. `rx.recv().await` yields the value
/// once the sender fires. Held by main; the worker ships the reply through
/// the sender half.
pub struct Reply<T> {
    pub(crate) rx: async_channel::Receiver<T>,
}

impl<T> Reply<T> {
    /// Create a paired (sender, receiver) slot pair backed by
    /// `async_channel::bounded(1)`.
    pub fn pair() -> (ReplySender<T>, Reply<T>) {
        let (tx, rx) = async_channel::bounded(1);
        (ReplySender { tx }, Reply { rx })
    }
}

impl<T> ReplySender<T> {
    /// Fire the reply. Consumes the sender (one-shot semantics). Uses
    /// `try_send` because the bounded(1) slot is always empty at fire
    /// time (main hasn't consumed yet — RPC replies are request/response,
    /// so main is awaiting on the receiver). Wakes the receiver.
    pub fn send(self, value: T) {
        let _ = self.tx.try_send(value);
    }
}

// Manual Debug impls — `PlatformEvent` / `DevNodeData` don't derive Debug,
// and the reply slots shouldn't print their payload.
impl fmt::Debug for WorkerMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlatformEvent(_) => f.debug_tuple("PlatformEvent").finish_non_exhaustive(),
            Self::RequestPaint => write!(f, "RequestPaint"),
            Self::Wake => write!(f, "Wake"),
            Self::LoadModule { source, .. } => f
                .debug_struct("LoadModule")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::LoadJs { source, .. } => f
                .debug_struct("LoadJs")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::EvalModule { source, .. } => f
                .debug_struct("EvalModule")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::EvalJs { source, .. } => f
                .debug_struct("EvalJs")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::DevElementTree { .. } => f.debug_struct("DevElementTree").finish(),
            Self::DevGetElement { id, .. } => {
                f.debug_struct("DevGetElement").field("id", id).finish()
            }
            Self::QueryFocusedState { .. } => f.debug_struct("QueryFocusedState").finish(),
            Self::QueryFocusedElement { .. } => f.debug_struct("QueryFocusedElement").finish(),
            Self::QueryFocusedCursorRect { .. } => {
                f.debug_struct("QueryFocusedCursorRect").finish()
            }
            Self::QueryFocusedIsEditable { .. } => {
                f.debug_struct("QueryFocusedIsEditable").finish()
            }
            Self::QueryElement { key, .. } => {
                f.debug_struct("QueryElement").field("key", &key).finish()
            }
            Self::EventBusToJs(bytes) => f.debug_tuple("EventBusToJs").field(&bytes.len()).finish(),
            Self::AppEvent(_) => f.debug_tuple("AppEvent").finish_non_exhaustive(),
            Self::RenderToPixels { .. } => f.debug_struct("RenderToPixels").finish(),
            Self::Destroy { .. } => write!(f, "Destroy"),
        }
    }
}

impl fmt::Debug for MainMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RenderCommands { commands, .. } => f
                .debug_tuple("RenderCommands")
                .field(&commands.len())
                .finish(),
            Self::FrameOutcome(fo) => f.debug_tuple("FrameOutcome").field(fo).finish(),
            Self::CursorChanged(c) => f.debug_tuple("CursorChanged").field(c).finish(),
            Self::FocusedStateChanged {
                is_editable,
                cursor_rect,
            } => f
                .debug_struct("FocusedStateChanged")
                .field("is_editable", is_editable)
                .field("cursor_rect", cursor_rect)
                .finish(),
            Self::EventBusToHost(bytes) => {
                f.debug_tuple("EventBusToHost").field(&bytes.len()).finish()
            }
            Self::DevReply(_) => f.debug_tuple("DevReply").finish_non_exhaustive(),
            Self::Destroyed => write!(f, "Destroyed"),
        }
    }
}

impl fmt::Debug for DevReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementTree(_) => f.debug_tuple("ElementTree").finish_non_exhaustive(),
            Self::GetElement(_) => f.debug_tuple("GetElement").finish_non_exhaustive(),
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
// the worker↔main channel contract. If these fail, a new field's type
// isn't Send and needs wrapping (typically `Arc<T>`).
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<WorkerMsg>();
    assert_send::<MainMsg>();
    assert_send::<DevReply>();
    assert_send::<ModuleError>();
    // Channels themselves must be Send + Sync.
    assert_send::<WorkerTx>();
    assert_send::<WorkerRx>();
    assert_send::<MainTx>();
    assert_send::<MainRx>();
};

#[cfg(test)]
mod tests {
    use super::*;

    /// `Arc<str>` is the canonical source carrier — verify it round-trips
    /// through `WorkerMsg::LoadModule` without clone friction.
    #[test]
    fn load_module_carries_arc_str() {
        let (tx, _rx) = Reply::<Result<(), ModuleError>>::pair();
        let source: Arc<str> = Arc::from("export const x = 1;");
        let msg = WorkerMsg::LoadModule { source, reply: tx };
        assert!(matches!(msg, WorkerMsg::LoadModule { .. }));
    }

    /// `ModuleError` Display strings are stable (used for diagnostics).
    #[test]
    fn module_error_display() {
        assert_eq!(
            ModuleError::Parse("syn".into()).to_string(),
            "JS parse error: syn"
        );
        assert_eq!(
            ModuleError::Eval("run".into()).to_string(),
            "JS evaluation error: run"
        );
        assert_eq!(ModuleError::WorkerGone.to_string(), "worker gone");
    }

    /// Reply slot pair — sender fires, receiver drains (via try_recv on
    /// the underlying async_channel).
    #[test]
    fn reply_slot_round_trip() {
        let (tx, rx) = Reply::<u32>::pair();
        assert!(rx.rx.try_recv().is_err());
        tx.send(42);
        assert_eq!(rx.rx.try_recv(), Ok(42));
    }

    /// A dropped sender (without firing) leaves the receiver empty.
    #[test]
    fn reply_slot_dropped_sender_leaves_none() {
        let (tx, rx) = Reply::<u32>::pair();
        drop(tx);
        assert!(rx.rx.try_recv().is_err());
    }
}
