//! Event bus plugin — bidirectional byte-channel between the Rust host and
//! the JS realm.
//!
//! Registered as part of `tur:std` (merged by `TurStdPlugin`). The JS-side
//! `eventBus` object exposes:
//! - `on(callback)` — register a callback invoked with a `Uint8Array` for each
//!   host→JS message.
//! - `send(Uint8Array)` — push a byte payload to the host-side handlers.
//!
//! The host-side [`EventBus`] wrapper (retrieved via `EventBus::of(&app)`)
//! exposes:
//! - `emit_to_js(Vec<u8>)` — push bytes to be delivered to JS `on` callbacks.
//! - `on_bus_event(handler)` — register a Rust handler invoked with `Vec<u8>`
//!   for each JS→host message.
//!
//! All queues use separate `RefCell`s so a host handler calling `emit_to_js`
//! (or a JS callback calling `send`) does not cause double-borrow panics.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

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
// Shared state
// ---------------------------------------------------------------------------

type HostHandler = Box<dyn FnMut(Vec<u8>)>;

pub struct EventBusInner {
    host_to_js: RefCell<VecDeque<Vec<u8>>>,
    js_to_host: RefCell<VecDeque<Vec<u8>>>,
    js_handlers: RefCell<Vec<JsFunction>>,
    host_handlers: RefCell<Vec<HostHandler>>,
}

impl EventBusInner {
    fn new() -> Self {
        Self {
            host_to_js: RefCell::new(VecDeque::new()),
            js_to_host: RefCell::new(VecDeque::new()),
            js_handlers: RefCell::new(Vec::new()),
            host_handlers: RefCell::new(Vec::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// Host-side wrapper
// ---------------------------------------------------------------------------

pub struct EventBus(Rc<EventBusInner>);

impl EventBus {
    pub fn of(app: &crate::TurApp) -> Option<EventBus> {
        app.instance_data::<EventBusInner>().map(EventBus)
    }

    pub fn emit_to_js(&self, payload: Vec<u8>) {
        self.0.host_to_js.borrow_mut().push_back(payload);
    }

    pub fn on_bus_event(&self, handler: impl FnMut(Vec<u8>) + 'static) {
        self.0.host_handlers.borrow_mut().push(Box::new(handler));
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        EventBus(self.0.clone())
    }
}

// ---------------------------------------------------------------------------
// Subsystem
// ---------------------------------------------------------------------------

pub struct HostBusSubsystem(Rc<EventBusInner>);

impl Subsystem for HostBusSubsystem {
    fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext) {
        let inner = self.0.clone();

        let host_msgs: Vec<Vec<u8>> = inner.host_to_js.borrow_mut().drain(..).collect();
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

        let js_msgs: Vec<Vec<u8>> = inner.js_to_host.borrow_mut().drain(..).collect();
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
    inner: Rc<EventBusInner>,
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
    caps.inner.js_to_host.borrow_mut().push_back(bytes);
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
// Install
// ---------------------------------------------------------------------------

pub fn install_event_bus(ctx: &mut PluginContext) -> Result<Vec<ConstEntry>, TurError> {
    let inner = Rc::new(EventBusInner::new());

    ctx.store_instance_data::<EventBusInner>(inner.clone());
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
