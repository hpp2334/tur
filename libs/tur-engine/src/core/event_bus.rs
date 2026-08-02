//! Event bus — bidirectional byte-channel between the Rust host and the JS
//! realm.
//!
//! The bus is engine infrastructure (always installed by `TurStdPlugin`),
//! so the type lives in `core` rather than `builtin_plugins`. The
//! registration code (`install_event_bus`) and the JS bridge closures live
//! here too; `TurStdPlugin::register` just calls `install_event_bus`.
//!
//! The JS-side `eventBus` object exposes:
//! - `on(callback)` — register a callback invoked with a `Uint8Array` for each
//!   host→JS message.
//! - `send(Uint8Array)` — push a byte payload to the host-side handlers.
//!
//! The host-side [`EventBus`] wrapper (retrieved via
//! [`TurApp::event_bus`](crate::TurApp::event_bus)) exposes:
//! - [`EventBus::emit_to_js`] — push bytes to be delivered to JS `on`
//!   callbacks on the next flush.
//! - [`EventBus::on_bus_event`] — register a Rust handler invoked with
//!   `Vec<u8>` for each JS→host message.
//!
//! Shared state lives directly on [`EventBus`] (no separate "inner" type);
//! all sides (host handle, JS bridge closures, the
//! [`HostBusSubsystem`]) hold `Rc<EventBus>`. Queues use separate `RefCell`s
//! so a host handler calling `emit_to_js` (or a JS callback calling `send`)
//! does not cause double-borrow panics.

use std::cell::RefCell;
use std::collections::VecDeque;
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

/// Cross-thread host→JS byte queue. Host pushes (from any thread); the
/// worker's `HostBusSubsystem` drains during flush.
pub type HostToJsQueue = Arc<Mutex<VecDeque<Vec<u8>>>>;

/// Cross-thread JS→host byte queue. JS pushes (via `eventBus.send`); the
/// worker's `HostBusSubsystem` drains during flush and invokes host
/// handlers.
pub type JsToHostQueue = Arc<Mutex<VecDeque<Vec<u8>>>>;

// ---------------------------------------------------------------------------
// Shared state (was `EventBusInner`)
// ---------------------------------------------------------------------------

type HostHandler = Box<dyn FnMut(Vec<u8>)>;

pub struct EventBus {
    host_to_js: HostToJsQueue,
    js_to_host: JsToHostQueue,
    /// JS-side handlers registered via `eventBus.on`. `RefCell` because
    /// boa's `JsFunction` is `!Send`/`!Sync` — these stay on the worker
    /// thread (the subsystem that invokes them runs there).
    js_handlers: RefCell<Vec<JsFunction>>,
    /// Host-side handlers registered via `on_bus_event`. Run during the
    /// worker's `HostBusSubsystem` flush.
    host_handlers: RefCell<Vec<HostHandler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            host_to_js: Arc::new(Mutex::new(VecDeque::new())),
            js_to_host: Arc::new(Mutex::new(VecDeque::new())),
            js_handlers: RefCell::new(Vec::new()),
            host_handlers: RefCell::new(Vec::new()),
        }
    }

    /// Construct with pre-existing cross-thread queues. Used by
    /// `ThreadedBackend` to share queues between the worker's
    /// `EventBus` and main's `EventBusHandle`.
    pub fn from_queues(host_to_js: HostToJsQueue, js_to_host: JsToHostQueue) -> Self {
        Self {
            host_to_js,
            js_to_host,
            js_handlers: RefCell::new(Vec::new()),
            host_handlers: RefCell::new(Vec::new()),
        }
    }

    /// Clone the cross-thread queues. The returned handle can be sent
    /// across threads (Arc<Mutex> is Send + Sync); the full EventBus
    /// stays on the worker thread.
    pub fn queues(&self) -> (HostToJsQueue, JsToHostQueue) {
        (self.host_to_js.clone(), self.js_to_host.clone())
    }

    /// Retrieve the engine's always-installed event bus. Phase 5 promotes
    /// this from an `Option<EventBus>` (`EventBus::of`) to a direct handle
    /// — the bus is unconditionally installed by `TurStdPlugin`, so the
    /// `Option` was always unwrappable in practice.
    ///
    /// Phase 5 keeps `of()` as a back-compat alias (it returns `Some(bus)`
    /// unconditionally) so existing embedder/test code keeps working; new
    /// code should use [`TurApp::event_bus`](crate::TurApp::event_bus)
    /// directly.
    pub fn of(app: &crate::TurApp) -> Option<Rc<EventBus>> {
        Some(app.event_bus())
    }

    pub fn emit_to_js(&self, payload: Vec<u8>) {
        self.host_to_js.lock().unwrap().push_back(payload);
    }

    pub fn on_bus_event(&self, handler: impl FnMut(Vec<u8>) + 'static) {
        self.host_handlers.borrow_mut().push(Box::new(handler));
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

#[derive(Clone)]
enum EventBusHandleInner {
    /// Inline mode — shared queues with the worker's `EventBus`.
    Queues(HostToJsQueue, JsToHostQueue),
    /// Threaded mode — ship via the worker's mpsc.
    Channel(std::sync::mpsc::Sender<crate::core::app::WorkerMsg>),
}

impl EventBusHandle {
    pub fn from_queues(host_to_js: HostToJsQueue, js_to_host: JsToHostQueue) -> Self {
        Self {
            inner: EventBusHandleInner::Queues(host_to_js, js_to_host),
        }
    }

    pub fn from_channel(worker_tx: std::sync::mpsc::Sender<crate::core::app::WorkerMsg>) -> Self {
        Self {
            inner: EventBusHandleInner::Channel(worker_tx),
        }
    }

    /// Push bytes to be delivered to JS `on` callbacks on the next flush.
    pub fn emit_to_js(&self, payload: Vec<u8>) {
        match &self.inner {
            EventBusHandleInner::Queues(h, _) => h.lock().unwrap().push_back(payload),
            EventBusHandleInner::Channel(tx) => {
                let _ = tx.send(crate::core::app::WorkerMsg::EventBusToJs(payload));
            }
        }
    }

    /// Drain pending JS→host messages. Returns empty in channel mode
    /// (handlers run on the worker).
    pub fn drain_js_to_host(&self) -> Vec<Vec<u8>> {
        match &self.inner {
            EventBusHandleInner::Queues(_, j) => {
                let mut q = j.lock().unwrap();
                q.drain(..).collect()
            }
            EventBusHandleInner::Channel(_) => Vec::new(),
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

        let host_msgs: Vec<Vec<u8>> = inner.host_to_js.lock().unwrap().drain(..).collect();
        if !host_msgs.is_empty() {
            let handlers = inner.js_handlers.borrow().clone();
            for msg in host_msgs {
                let u8a = match JsUint8Array::from_iter(msg, cx.boa) {
                    Ok(a) => JsValue::from(a),
                    Err(e) => {
                        tracing::error!("HostBus: failed to create Uint8Array: {e}");
                        continue;
                    }
                };
                for handler in &handlers {
                    let args: [JsValue; 1] = [u8a.clone()];
                    if let Err(e) = handler.call(&JsValue::undefined(), &args, cx.boa) {
                        tracing::error!("HostBus: JS handler error: {e}");
                    }
                }
            }
            cx.mark_dirty();
        }

        let js_msgs: Vec<Vec<u8>> = inner.js_to_host.lock().unwrap().drain(..).collect();
        if !js_msgs.is_empty() {
            let mut handlers = inner.host_handlers.borrow_mut();
            for msg in js_msgs {
                for handler in handlers.iter_mut() {
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
    let cb = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("eventBus.on: expected a function"))
    })?;
    let func = JsFunction::from_object(cb.clone()).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("eventBus.on: expected a function"))
    })?;
    caps.inner.js_handlers.borrow_mut().push(func);
    Ok(JsValue::undefined())
}

fn tur_event_bus_send(
    _this: &JsValue,
    args: &[JsValue],
    caps: &EventBusCaptures,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let v = args.get_or_undefined(0);
    let bytes = extract_bytes_from_value(v, ctx)?;
    caps.inner.js_to_host.lock().unwrap().push_back(bytes);
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
        .length(1)
        .name(js_string!("on"))
        .build();

    let send_fn = NativeFunction::from_copy_closure_with_captures(tur_event_bus_send, caps);
    let send_obj = FunctionObjectBuilder::new(ctx.boa_mut().realm(), send_fn)
        .length(1)
        .name(js_string!("send"))
        .build();

    let obj = boa_engine::object::JsObject::with_object_proto(ctx.boa_mut().intrinsics());
    let _ = obj.create_data_property(js_string!("on"), JsValue::from(on_obj), ctx.boa_mut());
    let _ = obj.create_data_property(js_string!("send"), JsValue::from(send_obj), ctx.boa_mut());

    Ok(vec![("eventBus", JsValue::from(obj))])
}
