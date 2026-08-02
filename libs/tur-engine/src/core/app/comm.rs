//! Worker ↔ main message vocabulary (the wire format for the upcoming
//! multi-threaded runtime in Phase 7+).
//!
//! Phase 4 introduces these types **without** wiring actual cross-thread
//! channels. The single-threaded engine continues to run inline; existing
//! pub fns (`push_platform_event`, `load_module`, …) become thin wrappers
//! that build a [`WorkerMsg`], call
//! [`TurApp::handle_worker_msg`](crate::TurApp::handle_worker_msg) on the
//! same thread, and unwrap the [`Reply`] synchronously. Phase 7 swaps that
//! inline call for a real `mpsc` send + worker task without changing the
//! wire types or the public method shapes.
//!
//! ## Why define the wire format before threads exist?
//!
//! - Lets Phase 5 (event bus handle / async APIs / escape-hatch removal)
//!   code against a stable target.
//! - Lets Phase 6's `Send + Sync` prep verify against real types.
//! - Keeps Phase 7's worker/main split a "wire the channels" change, not
//!   a "redesign every API" change.
//!
//! ## Reply slots
//!
//! [`Reply`]`<T>` / [`ReplySender`]`<T>` are a minimal `Arc<Mutex<Option<T>>>`
//! slot pair — no Condvar, no async runtime. The sender half fires once;
//! the receiver half `try_recv`s. **Phase 4** uses them in single-threaded
//! inline mode (the sender fires during `handle_worker_msg`, before the
//! caller calls `try_recv`). **Phase 7** will replace them with
//! `tokio::sync::oneshot` for cross-thread + `.await` support — the wire
//! type names stay the same.
//!
//! ## Send-ness
//!
//! Every variant is `Send` (verified by the compile-time assertion at the
//! bottom of this file). [`AppEvent`] is deliberately **not** carried here
//! yet — its `Box<dyn CustomAppEvent>` payload isn't `Send` until Phase 6
//! adds that bound. Phase 6 will introduce `WorkerMsg::AppEvent(AppEvent)`
//! once the bound is in place.

use std::fmt;
use std::sync::{Arc, Condvar, Mutex};

use crate::core::app::FrameOutcome;
use crate::core::element::NodeId;
use crate::core::elements::DevNodeData;
use crate::core::platform::Cursor;
use crate::core::platform::PlatformEvent;
use crate::core::render::RenderCommand;

/// main → worker. All input that can drive the engine flows through one of
/// these variants. Phase 7's worker task is `while let Some(msg) =
/// rx.blocking_recv() { app.handle_worker_msg(msg) }`.
pub enum WorkerMsg {
    /// DOM / JNI / winit platform event (pointer, key, wheel, IME, resize,
    /// …). Dispatched to subsystems via `handle_platform_event` on the next
    /// flush iteration.
    PlatformEvent(PlatformEvent),
    /// Mark the next frame for paint without enqueuing an event. Mirrors
    /// today's `TurApp::request_paint` — sets the `need_paint` flag.
    RequestPaint,
    /// Drive one flush iteration. Sent by main's rAF loop. Returns once
    /// the engine has reached quiescence for this frame; the worker then
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
    /// Dev-tool: full element-tree snapshot. Async RPC.
    DevElementTree {
        reply: ReplySender<Option<DevNodeData>>,
    },
    /// Dev-tool: single-node snapshot.
    DevGetElement {
        id: NodeId,
        reply: ReplySender<Option<DevNodeData>>,
    },
    /// Event bus — host → JS bytes. Worker forwards to the JS-side
    /// `__turEventBus.toJs` sink during the next flush.
    EventBusToJs(Vec<u8>),
    /// Initiate shutdown. Worker drains pending work, replies when safe
    /// to drop.
    Destroy { reply: ReplySender<()> },
}

/// worker → main. Emitted by the worker either during a flush
/// ([`MainMsg::RenderCommands`], [`MainMsg::FocusedStateChanged`]) or in
/// response to a [`WorkerMsg`] RPC ([`MainMsg::DevReply`]).
pub enum MainMsg {
    /// One frame's worth of paint state. Main applies the batch to its
    /// `MainTree` and plays it back into its renderer.
    RenderCommands(Vec<RenderCommand>),
    /// Schedule decision after a flush. Main arms the next rAF /
    /// `setTimeout` based on `schedule`. The `Err(String)` variant
    /// carries a flush error message (worker can't ship `TurError`
    /// directly because its `JsEval` variant holds a boa `JsError` which
    /// is `!Send` — main re-wraps as `TurError::Other`).
    FrameOutcome(Result<FrameOutcome, String>),
    /// Resolved cursor changed this frame (deduped: only emitted on
    /// change). Main forwards to its `CursorBackend`.
    CursorChanged(Cursor),
    /// Focused-element state changed (used by main for IME / caret
    /// placement on platforms where the IME target lives off-engine).
    /// Pushed once per change, not per frame.
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

/// One-shot reply slot — sender side. See [module docs](self) for the
/// Phase 4 → Phase 7 migration path.
pub struct ReplySender<T> {
    slot: Arc<(Mutex<Option<T>>, Condvar)>,
}

/// One-shot reply slot — receiver side. `try_recv` is the inline-mode
/// accessor (the sender fires synchronously during `handle_worker_msg`);
/// `recv` blocks until the sender fires (Phase 7's threaded backend).
pub struct Reply<T> {
    slot: Arc<(Mutex<Option<T>>, Condvar)>,
}

impl<T> Reply<T> {
    /// Create a paired (sender, receiver) slot pair.
    pub fn pair() -> (ReplySender<T>, Reply<T>) {
        let slot = Arc::new((Mutex::new(None), Condvar::new()));
        (ReplySender { slot: slot.clone() }, Reply { slot })
    }

    /// Non-blocking receive. Returns `None` if the sender hasn't fired yet,
    /// `Some(value)` if it has (the slot is drained).
    ///
    /// Inline mode: the sender always fires before the receiver polls
    /// (synchronous dispatch), so this never returns `None` unless the
    /// worker dropped the sender without replying.
    pub fn try_recv(&self) -> Option<T> {
        self.slot.0.lock().ok().and_then(|mut g| g.take())
    }

    /// Blocking receive. Waits until the sender fires, then returns the
    /// value. Used by `ThreadedBackend` to make the synchronous public API
    /// (`load_module`, `dev_tool_*`, etc.) wait for the worker's reply.
    ///
    /// Inline mode never calls this — `try_recv` works because dispatch
    /// is synchronous. If you call `recv` in inline mode, it blocks
    /// forever (the sender is on the same thread, waiting for you to
    /// finish).
    pub fn recv(self) -> T {
        let (lock, cvar) = &*self.slot;
        let mut g = lock.lock().expect("reply slot poisoned");
        while g.is_none() {
            g = cvar.wait(g).expect("reply slot poisoned");
        }
        g.take().expect("slot filled above")
    }
}

impl<T> ReplySender<T> {
    /// Fire the reply. Consumes the sender (one-shot semantics).
    /// Wakes any thread blocked in [`Reply::recv`].
    pub fn send(self, value: T) {
        if let Ok(mut g) = self.slot.0.lock() {
            *g = Some(value);
            self.slot.1.notify_one();
        }
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
            Self::DevElementTree { .. } => f.debug_struct("DevElementTree").finish(),
            Self::DevGetElement { id, .. } => {
                f.debug_struct("DevGetElement").field("id", id).finish()
            }
            Self::EventBusToJs(bytes) => f.debug_tuple("EventBusToJs").field(&bytes.len()).finish(),
            Self::Destroy { .. } => write!(f, "Destroy"),
        }
    }
}

impl fmt::Debug for MainMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RenderCommands(cmds) => {
                f.debug_tuple("RenderCommands").field(&cmds.len()).finish()
            }
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
// isn't Send and needs wrapping (typically `Arc<T>`). `PlatformEvent` /
// `AppEvent`'s `Box<dyn Custom…>` payloads aren't Send today; Phase 6
// tightens those bounds.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<WorkerMsg>();
    assert_send::<MainMsg>();
    assert_send::<DevReply>();
    assert_send::<ModuleError>();
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

    /// Reply slot pair — sender fires, receiver drains.
    #[test]
    fn reply_slot_round_trip() {
        let (tx, rx) = Reply::<u32>::pair();
        assert!(rx.try_recv().is_none());
        tx.send(42);
        assert_eq!(rx.try_recv(), Some(42));
        // Drained — subsequent reads return None.
        assert!(rx.try_recv().is_none());
    }

    /// A dropped sender (without firing) leaves the receiver empty.
    #[test]
    fn reply_slot_dropped_sender_leaves_none() {
        let (tx, rx) = Reply::<u32>::pair();
        drop(tx);
        assert!(rx.try_recv().is_none());
    }
}
