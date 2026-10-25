//! Event bus — the bidirectional multiplexed byte channel between the
//! Rust host (the embedder) and the rut module, keyed by
//! `channel_id: u64`. Pure MECHANISM (the layering law: no element
//! knowledge — a queue, a channel registry, and two rows beside the
//! store/ctx rows); the embedder face is [`EventBusHandle`] via
//! [`TurApp::event_bus_handle`](crate::TurApp::event_bus_handle).
//!
//! Always installed — [`install`] runs from `TurStdPlugin::register`; the
//! bus is engine infrastructure, no capability, no embedder opt-in. The
//! authored face is the `tur_event_kit` prelude (pushed by the standard
//! bundle beside these rows — core never fills preludes).
//!
//! Semantics (unchanged from the boa era):
//!
//! - **Point-to-point per channel** — a message targets exactly one
//!   channel; handlers registered on that channel fire, all others don't.
//!   No broadcast.
//! - **Silent drop** — a message to a channel with no handlers is dropped
//!   (standard pub/sub). Delivery is at-most-once, on the next flush.
//! - **Developer-managed correlation** — the payload is opaque bytes;
//!   whatever structure (ids, JSON, ...) it carries is the app's business.
//! - **Host→module** rides `WorkerMsg::EventBusToModule` (any thread →
//!   worker); **module→host** rides `HostMsg::EventBusToEmbedder` (worker →
//!   main, dispatched in `apply_msg`).
//!
//! Delivery mechanics (the mutation rail): the subsystem drains the
//! host→module queue in `flush_pre_layout` and enqueues each registered
//! handler's invocation onto the mutation queue (`MutationHandle` over the
//! seal id, a `Value::Bytes` payload) — the flush's mutation pass invokes
//! the sealed closure with the ctx, exactly as net-stream chunks deliver.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use rut_core::types::{TY_BYTES, TY_NIL, TY_U64};

use crate::core::app::{WorkerMsg, WorkerTx};
use crate::core::edgy::mutation::{
    MutationHandle, PendingMutationInvocationQueue, ValueArgs,
};
use crate::core::edgy::Value;
use crate::core::plugin::PluginRegisterContext;
use crate::core::rut_runtime::{RutPkgCx, mutation_of};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

// ---------------------------------------------------------------------------
// The host side — the embedder's handle
// ---------------------------------------------------------------------------

type HostHandler = Box<dyn FnMut(Vec<u8>) + Send>;

/// The embedder's handle over the host↔module bus. Cheap clone (the
/// handler map is `Arc`-shared); `Send` on the handler boxes so a handler
/// registered on any host thread fires on the drain thread. `emit_to_module`
/// re-arms the worker (`emit` = deliver on the next flush, even when the
/// loop was idle).
#[derive(Clone)]
pub struct EventBusHandle {
    worker_tx: WorkerTx,
    worker_wake: Rc<dyn Fn()>,
    host_handlers: Arc<Mutex<HashMap<u64, Vec<HostHandler>>>>,
}

impl EventBusHandle {
    pub(crate) fn new(worker_tx: WorkerTx, worker_wake: Rc<dyn Fn()>) -> Self {
        Self {
            worker_tx,
            worker_wake,
            host_handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Push bytes on `channel_id` to be delivered to the module's
    /// `event_on` handlers registered on `channel_id` on the next flush.
    /// Messages to a channel with no module handlers are silently dropped
    /// (the pub/sub law). Safe from any host thread; re-arms an idle
    /// worker so delivery is prompt.
    pub fn emit_to_module(&self, channel_id: u64, payload: Vec<u8>) {
        let _ = self
            .worker_tx
            .unbounded_send(WorkerMsg::EventBusToModule {
                channel_id,
                payload,
            });
        (self.worker_wake)();
    }

    /// Register a host-side handler for module→host messages on
    /// `channel_id`. Fires on the host thread when
    /// `HostMsg::EventBusToEmbedder` for the channel is drained (the
    /// looper's `apply_msg`); multiple handlers per channel fire in
    /// registration order.
    pub fn on_bus_event(&self, channel_id: u64, handler: impl FnMut(Vec<u8>) + Send + 'static) {
        self.host_handlers
            .lock()
            .unwrap()
            .entry(channel_id)
            .or_default()
            .push(Box::new(handler));
    }

    /// Dispatch module→host bytes on `channel_id` to the handlers
    /// registered on the channel. Called by `HostBackend::apply_msg` when
    /// it receives `HostMsg::EventBusToEmbedder`. A handler-less channel
    /// drops silently. Each handler receives a payload clone (one
    /// registration can't consume another's bytes).
    pub(crate) fn dispatch_to_host(&self, channel_id: u64, bytes: Vec<u8>) {
        let mut handlers = self.host_handlers.lock().unwrap();
        if let Some(channel_handlers) = handlers.get_mut(&channel_id) {
            for handler in channel_handlers.iter_mut() {
                handler(bytes.clone());
            }
        }
    }
}

impl std::fmt::Debug for EventBusHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBusHandle").finish_non_exhaustive()
    }
}

// Compile-time guard: the handler boxes must be `Send` for the
// `Arc<Mutex>` map to be shared across host threads (the handle itself is
// host-thread-only — the wake is an `Rc`, like every `TurApp` face).
const _: fn() = || {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<HashMap<u64, Vec<HostHandler>>>();
    assert_sync::<Arc<Mutex<HashMap<u64, Vec<HostHandler>>>>>();
};

// ---------------------------------------------------------------------------
// The worker side — the queue, the channel registry, the subsystem
// ---------------------------------------------------------------------------

/// One registered module-side handler: the sealed `Mutation<bytes>` (as a
/// mutation handle over the seal id) plus the queue + dirty flag it fires
/// through (cloned from the registering row's `RutHandles` — the
/// AnimationController pattern; the subsystem never borrows the VM).
struct BusHandler {
    mutation: MutationHandle<ValueArgs>,
    queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    dirty: Rc<std::cell::Cell<bool>>,
}

/// The worker-side bus state (plugin state, worker-thread only): the
/// host→module queue (`WorkerMsg::EventBusToModule` pushes; the subsystem
/// drains) and the channel → handlers registry (`event_on_raw` inserts).
pub struct EventBusState {
    to_module: RefCell<VecDeque<(u64, Vec<u8>)>>,
    handlers: RefCell<HashMap<u64, Vec<BusHandler>>>,
}

impl EventBusState {
    fn new() -> Self {
        Self {
            to_module: RefCell::new(VecDeque::new()),
            handlers: RefCell::new(HashMap::new()),
        }
    }

    /// Queue a host→module message for delivery on the next flush (the
    /// `WorkerMsg::EventBusToModule` arm; worker thread).
    pub fn push_to_module(&self, channel_id: u64, payload: Vec<u8>) {
        self.to_module.borrow_mut().push_back((channel_id, payload));
    }

    /// Register a sealed handler on `channel` (the `event_on_raw` row).
    fn register_handler(
        &self,
        channel_id: u64,
        seal: u64,
        queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        dirty: Rc<std::cell::Cell<bool>>,
    ) {
        self.handlers
            .borrow_mut()
            .entry(channel_id)
            .or_default()
            .push(BusHandler {
                mutation: MutationHandle::<ValueArgs>::new(mutation_of(seal)),
                queue,
                dirty,
            });
    }
}

/// The bus subsystem — drains the host→module queue each fixed-point
/// iteration and enqueues the channel handlers' invocations onto their
/// mutation queues. Messages for handler-less channels drop here (the
/// documented pub/sub law).
struct EventBusSubsystem {
    state: Rc<EventBusState>,
}

impl Subsystem for EventBusSubsystem {
    fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        // Snapshot + clear under one borrow; the invocations enqueue OUTSIDE
        // it (a handler's mutation queue is a different cell — no
        // re-entrancy).
        let msgs: Vec<(u64, Vec<u8>)> = self.state.to_module.borrow_mut().drain(..).collect();
        if msgs.is_empty() {
            return;
        }
        let mut fired = false;
        for (channel_id, payload) in msgs {
            let handlers = self.state.handlers.borrow();
            let Some(ch) = handlers.get(&channel_id) else {
                continue; // silent drop — no handlers on this channel
            };
            for h in ch {
                h.queue.borrow_mut().push(
                    h.mutation,
                    ValueArgs(vec![Value::Bytes(std::rc::Rc::from(
                        payload.clone().into_boxed_slice(),
                    ))]),
                );
                h.dirty.set(true);
                fired = true;
            }
        }
        if fired {
            // Handler writes land through the mutation pass — the tree must
            // re-resolve (the same signal the net-stream chunk path raises).
            cx.mark_dirty();
        }
    }
}

// ---------------------------------------------------------------------------
// Install — the state, the subsystem, and the two mechanism rows
// ---------------------------------------------------------------------------

/// Install the event bus mechanism: the worker-side state (plugin state),
/// the drain subsystem, and the `event_bus` rows (`event_on_raw` /
/// `event_send_raw` — mechanism rows beside the store/ctx rows). Called by
/// `TurStdPlugin::register`; the authored face (`tur_event_kit`) is pushed
/// as a prelude by the standard bundle assembly beside this.
pub fn install(ctx: &mut PluginRegisterContext) {
    let state = Rc::new(EventBusState::new());
    ctx.define_plugin_state::<EventBusState>(state.clone());
    ctx.register_subsystem(Box::new(EventBusSubsystem { state }));

    // The rows (the pkg-extension seam — the anim installer pattern).
    ctx.push_rut_ext(std::rc::Rc::new(|cx: &mut RutPkgCx<'_>| {
        let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
            crate::core::rut_runtime::host_row_at("event_bus", n, p, r, false)
        };
        cx.decl.extend(vec![
            // event_on_raw(channel, seal) — register the sealed bytes
            // mutation on the channel (the kit seals with SEAL_BYTES; 10).
            row("event_on_raw", vec![TY_U64, TY_U64], TY_NIL),
            // event_send_raw(channel, payload) — ship bytes to the
            // host-side handlers registered on the channel.
            row("event_send_raw", vec![TY_U64, TY_BYTES], TY_NIL),
        ]);

        let Some(handles) = cx.handles else {
            return;
        };
        let handles = handles.clone();
        let Some(state) = handles.inst.plugin_state::<EventBusState>() else {
            return;
        };
        let pkg = &mut *cx.pkg;

        // event_on_raw(channel, seal) — the sealed mutation + this row's
        // queue and dirty handles ride together into the channel registry
        // (the AnimationController pattern: the subsystem enqueues, the
        // flush's mutation pass invokes — the VM is never borrowed).
        let state_on = state.clone();
        let handles_on = handles.clone();
        rut_vm::pkg_fn!(pkg, "event_bus::event_on_raw", (u64, u64) -> (), move |_vm: &mut rut_vm::interp::Vm, channel: u64, seal: u64| {
            state_on.register_handler(
                channel,
                seal,
                handles_on.mutation_queue.clone(),
                handles_on.dirty.clone(),
            );
            Ok(())
        });

        // event_send_raw(channel, payload) — module→host: the bytes ship to
        // main on the shared worker→host channel (FIFO with the frame
        // messages); `apply_msg` dispatches to the handle's registered
        // handlers. A handler-less channel drops silently on the host side.
        let host_tx = handles.host_tx.clone();
        rut_vm::pkg_fn!(pkg, "event_bus::event_send_raw", (u64, &[u8]) -> (), move |_vm: &mut rut_vm::interp::Vm, channel: u64, payload: &[u8]| {
            let _ = host_tx.unbounded_send(crate::core::app::HostMsg::EventBusToEmbedder {
                channel_id: channel,
                payload: payload.to_vec(),
            });
            Ok(())
        });
    }));
}
