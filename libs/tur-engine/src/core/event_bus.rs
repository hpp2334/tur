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
//! The worker-side shared state lives on [`EventBus`] (held by the JS
//! bridge closures + the [`EmbedderBusSubsystem`]); the host-side
//! [`EventBusHandle`] holds the worker channel instead. Queues use separate
//! `RefCell`s so a host handler calling `emit_to_js` (or a JS callback
//! calling `send`) does not cause double-borrow panics.

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
use crate::core::plugin::PluginRegisterContext;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::error::TurError;

// ---------------------------------------------------------------------------
// Cross-thread queue types
// ---------------------------------------------------------------------------

/// Cross-thread host→JS byte queue. Host pushes `(channel_id, bytes)` (from
/// any thread); the worker's `EmbedderBusSubsystem` drains during flush and
/// routes each message to the JS handlers registered on `channel_id`.
pub type EmbedderToJsQueue = Arc<Mutex<VecDeque<(u64, Vec<u8>)>>>;

/// Cross-thread JS→host byte queue. JS pushes `(channel_id, bytes)` (via
/// `eventBus.send`); the worker's `EmbedderBusSubsystem` drains during flush and
/// invokes host handlers registered on `channel_id`.
pub type JsToEmbedderQueue = Arc<Mutex<VecDeque<(u64, Vec<u8>)>>>;

// ---------------------------------------------------------------------------
// Shared state (was `EventBusInner`)
// ---------------------------------------------------------------------------

type EmbedderHandler = Box<dyn FnMut(Vec<u8>)>;

pub struct EventBus {
    embedder_to_js: EmbedderToJsQueue,
    js_to_embedder: JsToEmbedderQueue,
    /// JS-side handlers registered via `eventBus.on`, keyed by `channel_id`.
    /// `RefCell` because boa's `JsFunction` is `!Send`/`!Sync` — these stay
    /// on the worker thread (the subsystem that invokes them runs there).
    js_handlers: RefCell<HashMap<u64, Vec<JsFunction>>>,
    /// Worker-side handlers registered via `EventBus::on_bus_event`,
    /// keyed by `channel_id`. Run during the worker's
    /// `EmbedderBusSubsystem` flush when no `host_tx` is set (handler
    /// registered on the worker itself, e.g. by a plugin — no `Send`
    /// bound). With `host_tx` set, bytes ship to main and host-side
    /// `EventBusHandle::on_bus_event` handlers fire instead.
    embedder_handlers: RefCell<HashMap<u64, Vec<EmbedderHandler>>>,
    /// Worker → main sender. When set, JS→host bytes are shipped to main
    /// via `HostMsg::EventBusToEmbedder` so handlers registered on the
    /// host-side `EventBusHandle` fire. Always set by
    /// `TurAppInternal::new`; `None` only for a standalone `EventBus::new`.
    host_tx: RefCell<Option<crate::core::app::HostTx>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            embedder_to_js: Arc::new(Mutex::new(VecDeque::new())),
            js_to_embedder: Arc::new(Mutex::new(VecDeque::new())),
            js_handlers: RefCell::new(HashMap::new()),
            embedder_handlers: RefCell::new(HashMap::new()),
            host_tx: RefCell::new(None),
        }
    }

    /// Retrieve the engine's cross-thread event bus handle. The full
    /// `EventBus` lives on the worker thread; this handle routes
    /// `emit_to_js` via the worker's channel.
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
        self.embedder_to_js
            .lock()
            .unwrap()
            .push_back((channel_id, payload));
    }

    /// Register a host-side handler for JS→host messages on `channel_id`.
    pub fn on_bus_event(&self, channel_id: u64, handler: impl FnMut(Vec<u8>) + 'static) {
        self.embedder_handlers
            .borrow_mut()
            .entry(channel_id)
            .or_default()
            .push(Box::new(handler));
    }

    /// Set the worker→host sender. When set, JS→host bytes are shipped to
    /// main via `HostMsg::EventBusToEmbedder` during `EmbedderBusSubsystem::flush`,
    /// so handlers registered on the host-side `EventBusHandle` fire.
    pub fn set_host_tx(&self, tx: crate::core::app::HostTx) {
        *self.host_tx.borrow_mut() = Some(tx);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Main-side handle — ships via the worker's channel
// ---------------------------------------------------------------------------

/// Host-side handler stored on the `EventBusHandle`.
type MainHostHandler = Box<dyn FnMut(Vec<u8>) + Send>;

/// Shared handler list keyed by `channel_id` — all clones of an
/// `EventBusHandle` see the same map, so a handler registered on one
/// clone fires when `HostBackend` dispatches on its own clone for the
/// matching `channel_id`.
type SharedHostHandlers = Arc<Mutex<HashMap<u64, Vec<MainHostHandler>>>>;

/// Main-side handle to the event bus. Holds the worker
/// `Sender<WorkerMsg>`: `emit_to_js` ships via `WorkerMsg::EventBusToJs`,
/// and JS→host messages come back as `HostMsg::EventBusToEmbedder`, which
/// `HostBackend` dispatches into the shared `embedder_handlers` map.
#[derive(Clone)]
pub struct EventBusHandle {
    worker_tx: crate::core::app::WorkerTx,
    /// Shared handler list keyed by `channel_id` — all clones of an
    /// `EventBusHandle` see the same map, so a handler registered on one
    /// clone fires when `HostBackend` dispatches on its own clone for the
    /// matching `channel_id`.
    embedder_handlers: SharedHostHandlers,
}

impl EventBusHandle {
    pub fn from_channel(worker_tx: crate::core::app::WorkerTx) -> Self {
        Self {
            worker_tx,
            embedder_handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Push bytes on `channel_id` to be delivered to JS `on` callbacks
    /// registered on `channel_id` on the next flush.
    pub fn emit_to_js(&self, channel_id: u64, payload: Vec<u8>) {
        let _ = self
            .worker_tx
            .unbounded_send(crate::core::app::WorkerMsg::EventBusToJs {
                channel_id,
                payload,
            });
    }

    /// Register a host-side handler for JS→host messages on `channel_id`.
    /// The handler is stored in a shared `Arc<Mutex>` and fires when
    /// `HostBackend` dispatches a `HostMsg::EventBusToEmbedder` for
    /// `channel_id`.
    pub fn on_bus_event(&self, channel_id: u64, handler: impl FnMut(Vec<u8>) + Send + 'static) {
        self.embedder_handlers
            .lock()
            .unwrap()
            .entry(channel_id)
            .or_default()
            .push(Box::new(handler));
    }

    /// Dispatch JS→host bytes on `channel_id` to handlers registered on
    /// `channel_id`. Called by `HostBackend` when it receives
    /// `HostMsg::EventBusToEmbedder`.
    pub(crate) fn dispatch_to_host(&self, channel_id: u64, bytes: Vec<u8>) {
        let mut handlers = self.embedder_handlers.lock().unwrap();
        if let Some(channel_handlers) = handlers.get_mut(&channel_id) {
            for handler in channel_handlers.iter_mut() {
                handler(bytes.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Subsystem — drains the queues each flush
// ---------------------------------------------------------------------------

pub struct EmbedderBusSubsystem(Rc<EventBus>);

impl Subsystem for EmbedderBusSubsystem {
    fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext) {
        let inner = self.0.clone();

        let host_msgs: Vec<(u64, Vec<u8>)> =
            inner.embedder_to_js.lock().unwrap().drain(..).collect();
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

        let js_msgs: Vec<(u64, Vec<u8>)> = inner.js_to_embedder.lock().unwrap().drain(..).collect();
        if !js_msgs.is_empty() {
            // Ship each message to main so handlers registered on the
            // host-side `EventBusHandle` fire.
            if let Some(tx) = inner.host_tx.borrow().as_ref() {
                for (channel_id, msg) in &js_msgs {
                    let _ = tx.unbounded_send(crate::core::app::HostMsg::EventBusToEmbedder {
                        channel_id: *channel_id,
                        payload: msg.clone(),
                    });
                }
            }
            // Worker-side handlers (registered on the `EventBus` itself,
            // e.g. by a plugin) run directly, filtered by `channel_id`.
            let mut handlers = inner.embedder_handlers.borrow_mut();
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
// The eventBus JS object — payload + plain fn-pointer methods
// ---------------------------------------------------------------------------

/// Payload of the `eventBus` JS object: the shared [`EventBus`] handle. The
/// `on` / `send` methods are plain fn pointers reading it off `this` — no
/// closures, no captures. Same `unsafe_empty_trace` soundness note as
/// `JsStore` (pure-Rust state, no `Gc`).
#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
struct EventBusState {
    inner: Rc<EventBus>,
}

/// `this` → the shared `EventBus`, cloned out before any JS runs.
fn event_bus_of(this: &JsValue) -> JsResult<Rc<EventBus>> {
    let msg = "expected the eventBus object as `this` — call it as a method (eventBus.on(...))";
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message(msg)))?;
    obj.downcast_ref::<EventBusState>()
        .map(|s| s.inner.clone())
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message(msg)))
}

// ---------------------------------------------------------------------------
// Bridge fns
// ---------------------------------------------------------------------------

fn tur_event_bus_on(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let inner = event_bus_of(this)?;
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
    inner
        .js_handlers
        .borrow_mut()
        .entry(channel_id)
        .or_default()
        .push(func);
    Ok(JsValue::undefined())
}

fn tur_event_bus_send(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let inner = event_bus_of(this)?;
    let channel_id =
        args.get_or_undefined(0).as_number().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message(
                "eventBus.send: expected a channel_id (number) as the first argument",
            ))
        })? as u64;
    let bytes = extract_bytes_from_value(args.get_or_undefined(1), ctx)?;
    inner
        .js_to_embedder
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

/// Wire up the event bus: register the [`EmbedderBusSubsystem`] (drains queues
/// each flush) and the JS-side `eventBus` object (`on`/`send`). The shared
/// state is created up-front in [`crate::core::app::TurAppInternal::new`]
/// and exposed to plugins via
/// [`PluginRegisterContext::event_bus`](crate::core::plugin::PluginRegisterContext::event_bus);
/// this function just hooks up the JS bridge + subsystem to that shared
/// state.
pub fn install_event_bus(ctx: &mut PluginRegisterContext) -> Result<Vec<ConstEntry>, TurError> {
    let inner = ctx.event_bus();

    ctx.register_subsystem(Box::new(EmbedderBusSubsystem(inner.clone())));

    let on_obj = FunctionObjectBuilder::new(
        ctx.boa_mut().realm(),
        NativeFunction::from_fn_ptr(tur_event_bus_on),
    )
    .length(2)
    .name(js_string!("on"))
    .build();

    let send_obj = FunctionObjectBuilder::new(
        ctx.boa_mut().realm(),
        NativeFunction::from_fn_ptr(tur_event_bus_send),
    )
    .length(2)
    .name(js_string!("send"))
    .build();

    let obj = boa_engine::object::JsObject::from_proto_and_data(
        ctx.boa_mut()
            .intrinsics()
            .constructors()
            .object()
            .prototype(),
        EventBusState {
            inner: inner.clone(),
        },
    );
    let _ = obj.create_data_property(js_string!("on"), JsValue::from(on_obj), ctx.boa_mut());
    let _ = obj.create_data_property(js_string!("send"), JsValue::from(send_obj), ctx.boa_mut());

    Ok(vec![("eventBus", JsValue::from(obj))])
}
