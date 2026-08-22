//! Worker ↔ host message vocabulary.
//!
//! The engine runs on a worker thread (see [`crate::core::runtime::HostBackend`]);
//! the embedder drives it from the host thread via [`WorkerMsg`]s and receives
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
    /// Parse + load + evaluate a JS module, then invoke its `start()`
    /// export (the module lifecycle contract: `start` returns an optional
    /// cleanup function that runs before the next load or at destroy).
    /// Reply carries the parse/eval outcome. `Arc<str>` because module
    /// sources can be large (the playground ships multi-KB compiled JS) —
    /// `Arc` lets the message be duplicated cheaply if needed (e.g.
    /// dev-tool logging).
    LoadModule {
        source: Arc<str>,
        reply: ReplySender<Result<(), ModuleError>>,
    },
    /// Synchronous JS expression evaluation (test-only). Runs `ctx.eval(source)`
    /// on the worker, converts the result to its display string, and replies.
    /// Production code uses `LoadModule`; this is for tests
    /// that read JS-side state via `globalThis.__x = ...`.
    EvalJs {
        source: Arc<str>,
        reply: ReplySender<String>,
    },
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
    /// Event bus — embedder → JS bytes on `channel_id`. Worker pushes into the
    /// `EventBus` `embedder_to_js` queue; `EmbedderBusSubsystem` drains it on the
    /// next flush and delivers to JS `eventBus.on` callbacks registered on
    /// `channel_id`.
    EventBusToJs { channel_id: u64, payload: Vec<u8> },
    /// Push an engine-internal event (programmatic scroll, clipboard
    /// write, etc.).
    AppEvent(crate::core::app::AppEvent),
    /// Initiate shutdown. Worker drains pending work, replies when safe
    /// to drop.
    Destroy { reply: ReplySender<()> },
}

/// worker → host. Emitted by the worker either during a flush
/// ([`HostMsg::RenderCommands`], [`HostMsg::Shell`]) or in response to a
/// [`WorkerMsg`] RPC (`Reply<T>` slots).
pub enum HostMsg {
    /// One frame's worth of paint state. Main applies the batch to its
    /// renderer (owned by `HostBackend`) directly.
    ///
    /// Images are NOT shipped here — they travel once per new resource via
    /// [`HostMsg::UploadImage`] (main uploads them into its atlas
    /// incrementally). Resizes travel via [`HostMsg::Resized`] (main calls
    /// `renderer.resize(...)` only when the viewport actually changes).
    RenderCommands { commands: Vec<RenderCommand> },
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
    /// Event bus — JS → embedder bytes on `channel_id`. Worker ships one
    /// `HostMsg` per `eventBus.send` dispatch; `HostBackend` dispatches to
    /// handlers registered on `channel_id` on the host-side
    /// `EventBusHandle`.
    EventBusToEmbedder { channel_id: u64, payload: Vec<u8> },
    /// Worker finished shutting down (response to `WorkerMsg::Destroy`).
    Destroyed,
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
            Self::LoadModule { source, .. } => f
                .debug_struct("LoadModule")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::EvalJs { source, .. } => f
                .debug_struct("EvalJs")
                .field("source_len", &source.len())
                .finish_non_exhaustive(),
            Self::WithTree { .. } => f.debug_struct("WithTree").finish(),
            Self::EventBusToJs {
                channel_id,
                payload,
            } => f
                .debug_struct("EventBusToJs")
                .field("channel_id", channel_id)
                .field("len", &payload.len())
                .finish(),
            Self::AppEvent(_) => f.debug_tuple("AppEvent").finish_non_exhaustive(),
            Self::Destroy { .. } => write!(f, "Destroy"),
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
            Self::EventBusToEmbedder {
                channel_id,
                payload,
            } => f
                .debug_struct("EventBusToEmbedder")
                .field("channel_id", channel_id)
                .field("len", &payload.len())
                .finish(),
            Self::Destroyed => write!(f, "Destroyed"),
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
