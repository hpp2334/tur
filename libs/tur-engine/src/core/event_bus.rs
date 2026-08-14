//! Event bus — bidirectional multiplexed byte-channel between the Rust host
//! and the JS realm, keyed by `channel_id: u64`.
//!
//! The bus is engine infrastructure (always installed by `TurStdPlugin`),
//! so the type lives in `core` rather than `builtin_plugins`. The
//! registration code (`install_event_bus`) and the JS bridge closures live
//! here too; `TurStdPlugin::register` just calls `install_event_bus`.
//!
//! Every message carries a `channel_id`. A handler registered on channel `N`
//! only receives messages sent/emitted on channel `N` — there is no
//! broadcast: messages target exactly one channel, and a message to a
//! channel with no handlers is silently dropped (standard pub/sub).
//!
//! The JS-side `eventBus` object exposes:
//! - `on(channelId, callback)` — register a callback invoked with a
//!   `Uint8Array` for each host→JS message on `channelId`.
//! - `send(channelId, Uint8Array)` — push a byte payload to the host-side
//!   handlers registered on `channelId`.
//!
//! The host-side [`EventBus`] wrapper (retrieved via
//! [`TurApp::event_bus`](crate::TurApp::event_bus)) exposes:
//! - [`EventBus::emit_to_js`] — push bytes on a channel to be delivered to
//!   JS `on` callbacks (registered on that channel) on the next flush.
//! - [`EventBus::on_bus_event`] — register a Rust handler invoked with
//!   `Vec<u8>` for each JS→host message on a channel.
//!
//! Shared state lives directly on [`EventBus`] (no separate "inner" type);
//! all sides (host handle, JS bridge closures, the
//! [`HostBusSubsystem`]) hold `Rc<EventBus>`. Queues use separate `RefCell`s
//! so a host handler calling `emit_to_js` (or a JS callback calling `send`)
//! does not cause double-borrow panics.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use boa_engine::object::FunctionObjectBuilder;
use boa_engine::object::builtins::{JsFunction, JsUint8Array};
use boa_engine::{
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction, js_string,
};
use boa_gc::{Finalize, Trace};

use crate::core::js_runtime::helpers::ConstEntry;
use crate::core::plugin::PluginContext;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::error::TurError;

// ---------------------------------------------------------------------------
// Cross-thread queue types
// ---------------------------------------------------------------------------

/// Cross-thread host→JS byte queue. Host pushes `(channel_id, bytes)` (from
/// any thread); the worker's `HostBusSubsystem` drains during flush and
/// routes each message to the JS handlers registered on `channel_id`.
pub type HostToJsQueue = Arc<Mutex<VecDeque<(u64, Vec<u8>)>>>;

/// Cross-thread JS→host byte queue. JS pushes `(channel_id, bytes)` (via
/// `eventBus.send`); the worker's `HostBusSubsystem` drains during flush and
/// invokes host handlers registered on `channel_id`.
pub type JsToHostQueue = Arc<Mutex<VecDeque<(u64, Vec<u8>)>>>;

// ---------------------------------------------------------------------------
// Shared state (was `EventBusInner`)
// ---------------------------------------------------------------------------

type HostHandler = Box<dyn FnMut(Vec<u8>)>;

pub struct EventBus {
    host_to_js: HostToJsQueue,
    js_to_host: JsToHostQueue,
    /// JS-side handlers registered via `eventBus.on`, keyed by `channel_id`.
    /// `RefCell` because boa's `JsFunction` is `!Send`/`!Sync` — these stay
    /// on the worker thread (the subsystem that invokes them runs there).
    js_handlers: RefCell<HashMap<u64, Vec<JsFunction>>>,
    /// Host-side handlers registered via `on_bus_event`, keyed by
    /// `channel_id`. Run during the worker's `HostBusSubsystem` flush
    /// (inline mode only — threaded mode ships bytes to main via `main_tx`).
    host_handlers: RefCell<HashMap<u64, Vec<HostHandler>>>,
    /// Worker → main sender. When set (threaded mode), JS→host bytes are
    /// shipped to main via `MainMsg::EventBusToHost` so handlers registered
    /// on the main-side `EventBusHandle` fire. `None` in inline mode.
    main_tx: RefCell<Option<crate::core::app::MainTx>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            host_to_js: Arc::new(Mutex::new(VecDeque::new())),
            js_to_host: Arc::new(Mutex::new(VecDeque::new())),
            js_handlers: RefCell::new(HashMap::new()),
            host_handlers: RefCell::new(HashMap::new()),
            main_tx: RefCell::new(None),
        }
    }

    /// Construct with pre-existing cross-thread queues. Used by
    /// `ThreadedBackend` to share queues between the worker's
    /// `EventBus` and main's `EventBusHandle`.
    pub fn from_queues(host_to_js: HostToJsQueue, js_to_host: JsToHostQueue) -> Self {
        Self {
            host_to_js,
            js_to_host,
            js_handlers: RefCell::new(HashMap::new()),
            host_handlers: RefCell::new(HashMap::new()),
            main_tx: RefCell::new(None),
        }
    }

    /// Clone the cross-thread queues. The returned handle can be sent
    /// across threads (Arc<Mutex> is Send + Sync); the full EventBus
    /// stays on the worker thread.
    pub fn queues(&self) -> (HostToJsQueue, JsToHostQueue) {
        (self.host_to_js.clone(), self.js_to_host.clone())
    }

    /// Retrieve the engine's cross-thread event bus handle. The full
    /// `EventBus` lives on the worker thread; this handle routes
    /// `emit_to_js` via the worker's channel and exposes `drain_js_to_host`
    /// for tests that drive the worker inline (no thread).
    ///
    /// Production code uses [`TurApp::event_bus_handle`](crate::TurApp::event_bus_handle)
    /// directly. This alias is kept for back-compat with code that imported
    /// `EventBus` rather than reaching through `TurApp`.
    pub fn of(app: &crate::TurApp) -> Option<EventBusHandle> {
        Some(app.event_bus_handle())
    }

    /// Push bytes on `channel_id` to be delivered to JS `on` callbacks
    /// registered on `channel_id` on the next flush. Messages to a channel
    /// with no JS handlers are silently dropped.
    pub fn emit_to_js(&self, channel_id: u64, payload: Vec<u8>) {
        self.host_to_js
            .lock()
            .unwrap()
            .push_back((channel_id, payload));
    }

    /// Register a host-side handler for JS→host messages on `channel_id`.
    pub fn on_bus_event(&self, channel_id: u64, handler: impl FnMut(Vec<u8>) + 'static) {
        self.host_handlers
            .borrow_mut()
            .entry(channel_id)
            .or_default()
            .push(Box::new(handler));
    }

    /// Set the worker→main sender. When set, JS→host bytes are shipped to
    /// main via `MainMsg::EventBusToHost` during `HostBusSubsystem::flush`,
    /// so handlers registered on the main-side `EventBusHandle` fire.
    pub fn set_main_tx(&self, tx: crate::core::app::MainTx) {
        *self.main_tx.borrow_mut() = Some(tx);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Main-side handle (for threaded mode) — owns just the queues
// ---------------------------------------------------------------------------

/// Main-side handle to the event bus. Holds either:
/// - **Queues mode** (inline): direct `Arc<Mutex<>>` queue clones,
///   shared with the worker's `EventBus`. Full functionality.
/// - **Channel mode** (threaded): a worker `Sender<WorkerMsg>`.
///   `emit_to_js` ships via `WorkerMsg::EventBusToJs`. `drain_js_to_host`
///   returns empty (handlers run on worker).
///
/// Both modes make `emit_to_js` work cross-thread, which is the embedder
/// hot path. Use `TurApp::event_bus_handle()` for cross-thread access;
/// use `TurApp::event_bus()` for inline-only full API
/// (`on_bus_event` etc.).
#[derive(Clone)]
pub struct EventBusHandle {
    inner: EventBusHandleInner,
}

/// Host-side handler stored on the main-side `EventBusHandle` (channel mode).
type MainHostHandler = Box<dyn FnMut(Vec<u8>) + Send>;

/// Shared handler list keyed by `channel_id` — all clones of an
/// `EventBusHandle` in channel mode see the same map, so a handler
/// registered on one clone fires when `MainBackend` dispatches on its own
/// clone for the matching `channel_id`.
type SharedHostHandlers = Arc<Mutex<HashMap<u64, Vec<MainHostHandler>>>>;

#[derive(Clone)]
enum EventBusHandleInner {
    /// Inline mode — shared queues with the worker's `EventBus`.
    Queues(HostToJsQueue, JsToHostQueue),
    /// Threaded mode — ship via the worker's `futures::channel` sender.
    /// `host_handlers` is shared across all clones (Arc<Mutex>) so a handler
    /// registered on one clone fires when `MainBackend` dispatches a
    /// `MainMsg::EventBusToHost` on its own clone.
    Channel {
        worker_tx: crate::core::app::WorkerTx,
        host_handlers: SharedHostHandlers,
    },
}

impl EventBusHandle {
    pub fn from_queues(host_to_js: HostToJsQueue, js_to_host: JsToHostQueue) -> Self {
        Self {
            inner: EventBusHandleInner::Queues(host_to_js, js_to_host),
        }
    }

    pub fn from_channel(worker_tx: crate::core::app::WorkerTx) -> Self {
        Self {
            inner: EventBusHandleInner::Channel {
                worker_tx,
                host_handlers: Arc::new(Mutex::new(HashMap::new())),
            },
        }
    }

    /// Push bytes on `channel_id` to be delivered to JS `on` callbacks
    /// registered on `channel_id` on the next flush.
    pub fn emit_to_js(&self, channel_id: u64, payload: Vec<u8>) {
        match &self.inner {
            EventBusHandleInner::Queues(h, _) => h.lock().unwrap().push_back((channel_id, payload)),
            EventBusHandleInner::Channel { worker_tx, .. } => {
                let _ = worker_tx.unbounded_send(crate::core::app::WorkerMsg::EventBusToJs {
                    channel_id,
                    payload,
                });
            }
        }
    }

    /// Register a host-side handler for JS→host messages on `channel_id`.
    /// In channel mode the handler is stored in a shared `Arc<Mutex>` and
    /// fires when `MainBackend` dispatches a `MainMsg::EventBusToHost` for
    /// `channel_id`.
    pub fn on_bus_event(&self, channel_id: u64, handler: impl FnMut(Vec<u8>) + Send + 'static) {
        if let EventBusHandleInner::Channel { host_handlers, .. } = &self.inner {
            host_handlers
                .lock()
                .unwrap()
                .entry(channel_id)
                .or_default()
                .push(Box::new(handler));
        }
    }

    /// Dispatch JS→host bytes on `channel_id` to handlers registered on
    /// `channel_id`. Called by `MainBackend` when it receives
    /// `MainMsg::EventBusToHost`.
    pub(crate) fn dispatch_to_host(&self, channel_id: u64, bytes: Vec<u8>) {
        if let EventBusHandleInner::Channel { host_handlers, .. } = &self.inner {
            let mut handlers = host_handlers.lock().unwrap();
            if let Some(channel_handlers) = handlers.get_mut(&channel_id) {
                for handler in channel_handlers.iter_mut() {
                    handler(bytes.clone());
                }
            }
        }
    }

    /// Drain pending JS→host messages (as `(channel_id, bytes)`). Returns
    /// empty in channel mode (handlers run on the worker).
    pub fn drain_js_to_host(&self) -> Vec<(u64, Vec<u8>)> {
        match &self.inner {
            EventBusHandleInner::Queues(_, j) => {
                let mut q = j.lock().unwrap();
                q.drain(..).collect()
            }
            EventBusHandleInner::Channel { .. } => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Subsystem — drains the queues each flush
// ---------------------------------------------------------------------------

pub struct HostBusSubsystem(Rc<EventBus>);

impl Subsystem for HostBusSubsystem {
    fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext) {
        let inner = self.0.clone();

        let host_msgs: Vec<(u64, Vec<u8>)> = inner.host_to_js.lock().unwrap().drain(..).collect();
        if !host_msgs.is_empty() {
            // Snapshot the handler map so JS callbacks calling `on`/`send`
            // during dispatch don't cause a double-borrow of the RefCell.
            let handlers_snapshot = inner.js_handlers.borrow().clone();
            for (channel_id, msg) in host_msgs {
                let Some(channel_handlers) = handlers_snapshot.get(&channel_id) else {
                    continue;
                };
                let u8a = match JsUint8Array::from_iter(msg, cx.boa) {
                    Ok(a) => JsValue::from(a),
                    Err(e) => {
                        tracing::error!("HostBus: failed to create Uint8Array: {e}");
                        continue;
                    }
                };
                for handler in channel_handlers {
                    let args: [JsValue; 1] = [u8a.clone()];
                    if let Err(e) = handler.call(&JsValue::undefined(), &args, cx.boa) {
                        tracing::error!("HostBus: JS handler error: {e}");
                    }
                }
            }
            cx.mark_dirty();
        }

        let js_msgs: Vec<(u64, Vec<u8>)> = inner.js_to_host.lock().unwrap().drain(..).collect();
        if !js_msgs.is_empty() {
            // Threaded mode: ship each message to main so handlers
            // registered on the main-side `EventBusHandle` fire.
            if let Some(tx) = inner.main_tx.borrow().as_ref() {
                for (channel_id, msg) in &js_msgs {
                    let _ = tx.unbounded_send(crate::core::app::MainMsg::EventBusToHost {
                        channel_id: *channel_id,
                        payload: msg.clone(),
                    });
                }
            }
            // Inline mode: call worker-side handlers directly, filtered by
            // channel_id.
            let mut handlers = inner.host_handlers.borrow_mut();
            for (channel_id, msg) in js_msgs {
                let Some(channel_handlers) = handlers.get_mut(&channel_id) else {
                    continue;
                };
                for handler in channel_handlers.iter_mut() {
                    handler(msg.clone());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Captures for the bridge closures (held inside boa's GC heap)
// ---------------------------------------------------------------------------

#[derive(Clone, Trace, Finalize)]
#[boa_gc(unsafe_empty_trace)]
struct EventBusCaptures {
    inner: Rc<EventBus>,
}

// ---------------------------------------------------------------------------
// Bridge fns
// ---------------------------------------------------------------------------

fn tur_event_bus_on(
    _this: &JsValue,
    args: &[JsValue],
    caps: &EventBusCaptures,
    _ctx: &mut Context,
) -> JsResult<JsValue> {
    let channel_id = args.get_or_undefined(0).as_number().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("eventBus.on: expected a channel_id (number) as the first argument"),
        )
    })? as u64;
    let cb = args.get_or_undefined(1).as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("eventBus.on: expected a function as the second argument"),
        )
    })?;
    let func = JsFunction::from_object(cb.clone()).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("eventBus.on: expected a function as the second argument"),
        )
    })?;
    caps.inner
        .js_handlers
        .borrow_mut()
        .entry(channel_id)
        .or_default()
        .push(func);
    Ok(JsValue::undefined())
}

fn tur_event_bus_send(
    _this: &JsValue,
    args: &[JsValue],
    caps: &EventBusCaptures,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let channel_id =
        args.get_or_undefined(0).as_number().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message(
                "eventBus.send: expected a channel_id (number) as the first argument",
            ))
        })? as u64;
    let bytes = extract_bytes_from_value(args.get_or_undefined(1), ctx)?;
    caps.inner
        .js_to_host
        .lock()
        .unwrap()
        .push_back((channel_id, bytes));
    Ok(JsValue::undefined())
}

fn extract_bytes_from_value(v: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
    use boa_engine::object::builtins::{JsArrayBuffer, JsTypedArray};
    let obj = v.as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("eventBus.send: expected Uint8Array or ArrayBuffer"),
        )
    })?;
    if let Ok(ta) = JsTypedArray::from_object(obj.clone()) {
        let offset = ta.byte_offset(ctx).unwrap_or(0);
        let len = ta.byte_length(ctx).unwrap_or(0);
        let buf_val = ta.buffer(ctx)?;
        let ab = JsArrayBuffer::from_object(buf_val.as_object().unwrap().clone())?;
        let full = ab.to_vec().unwrap_or_default();
        return Ok(full[offset..offset + len].to_vec());
    }
    if let Ok(ab) = JsArrayBuffer::from_object(obj.clone()) {
        return Ok(ab.to_vec().unwrap_or_default());
    }
    Err(JsError::from(JsNativeError::typ().with_message(
        "eventBus.send: expected Uint8Array or ArrayBuffer",
    )))
}

// ---------------------------------------------------------------------------
// Install — called by TurStdPlugin::register
// ---------------------------------------------------------------------------

/// Wire up the event bus: register the [`HostBusSubsystem`] (drains queues
/// each flush) and the JS-side `eventBus` object (`on`/`send`). The shared
/// state is created up-front in [`crate::core::app::TurAppInternal::new`]
/// and exposed to plugins via
/// [`PluginContext::event_bus`](crate::core::plugin::PluginContext::event_bus);
/// this function just hooks up the JS bridge + subsystem to that shared
/// state.
pub fn install_event_bus(ctx: &mut PluginContext) -> Result<Vec<ConstEntry>, TurError> {
    let inner = ctx.event_bus();

    ctx.register_subsystem(Box::new(HostBusSubsystem(inner.clone())));

    let caps = EventBusCaptures {
        inner: inner.clone(),
    };

    let on_fn = NativeFunction::from_copy_closure_with_captures(tur_event_bus_on, caps.clone());
    let on_obj = FunctionObjectBuilder::new(ctx.boa_mut().realm(), on_fn)
        .length(2)
        .name(js_string!("on"))
        .build();

    let send_fn = NativeFunction::from_copy_closure_with_captures(tur_event_bus_send, caps);
    let send_obj = FunctionObjectBuilder::new(ctx.boa_mut().realm(), send_fn)
        .length(2)
        .name(js_string!("send"))
        .build();

    let obj = boa_engine::object::JsObject::with_object_proto(ctx.boa_mut().intrinsics());
    let _ = obj.create_data_property(js_string!("on"), JsValue::from(on_obj), ctx.boa_mut());
    let _ = obj.create_data_property(js_string!("send"), JsValue::from(send_obj), ctx.boa_mut());

    Ok(vec![("eventBus", JsValue::from(obj))])
}
