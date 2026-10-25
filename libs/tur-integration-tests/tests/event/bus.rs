//! Integration tests for the event bus (`tur_event_kit`'s `event_on` /
//! `event_send` + the embedder's `EventBusHandle`).
//!
//! Validates bidirectional multiplexed byte-channel communication between
//! the Rust host and the rut module, keyed by `channel_id` — the rut-era
//! port of the boa-era `element::event_bus` suite (deleted with the JS
//! rail):
//! - Host→module: `EventBusHandle::emit_to_module(channel, bytes)` → the
//!   module's `event_on(channel, ..)` handler (next-flush delivery).
//! - Module→host: the module's `event_send(channel, bytes)` → the host
//!   handler registered via `EventBusHandle::on_bus_event(channel, ..)`.
//! - Multiple messages in both directions.
//! - Channel isolation: a message on channel N only fires handlers on N.
//! - Silent drop: a message to a handler-less channel is dropped (standard
//!   pub/sub — no broadcast, no error).
//! - Byte fidelity: the payload round-trips host→module→host unchanged.
//! - Late registration: a handler bound after boot receives the next
//!   message on its channel.
//!
//! The module fixture logs deliveries into a bound Text (the `bus/log`
//! query key — the same probe rail the corpus cases use); the host side
//! collects received bytes into a shared vec. The suite is THREE journeys
//! (one app each — a fresh app compiles the fixture, ~1.7s a ride; the
//! CI test budget is tight) covering the same ground the eight-test shape
//! did: the assertions below are the union.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tur_engine::builtin_plugins::text::elements::TextElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// The bus fixture module — the rails under test. Handlers log `cN`
/// markers into one bound Text; channel 7 also forwards every payload back
/// to the host on channel 9 (the round-trip rail). Channel 12 binds only
/// when the embedder calls the `late_bind` entry.
const FIXTURE: &str = r#"
use tur_event_kit::{ event_on, event_send };
use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::reactive::{ MutationCtx, Source, mutate, source };
use tur_kit::text::core::{ Text };

struct App {
    log: Source<str>,
}

fn boot() -> App {
    let log = source<str>("");

    // Channel 7: log the delivery and forward the payload to the host on
    // channel 9 (the round-trip rail — byte fidelity both directions).
    event_on(7, mutate<bytes>(fn (ctx: MutationCtx, payload: bytes) {
        let prev = ctx.get<str>(log);
        ctx.set<str>(log, f"{prev}c7 ");
        event_send(9, payload);
    }));

    // Channel 8: log only (the isolation channel).
    event_on(8, mutate<bytes>(fn (ctx: MutationCtx, _payload: bytes) {
        let prev = ctx.get<str>(log);
        ctx.set<str>(log, f"{prev}c8 ");
    }));

    mount(
        Container()
            .query_key("bus/log")
            .child(Text().text(log).query_key("bus/log-text").build())
            .build(),
    );
    return App { log };
}

// The boot defers to the embedder's first probe (the fixture contract):
// `entry_start` runs `boot` in the entry frame — the handlers mint and the
// tree mounts there; the boxed App is the context token.
entry fn entry_start() -> opaque {
    let cx = boot();
    return opaque(cx);
}

// Late registration — channel 12 binds only when the embedder calls this
// (the context-crossing lane: the cx token feeds the log source).
entry fn late_bind(cx: opaque, _n: u64) {
    let app = opaque::downcast<App>(cx);
    let log = app.log;
    event_on(12, mutate<bytes>(fn (ctx: MutationCtx, _payload: bytes) {
        let prev = ctx.get<str>(log);
        ctx.set<str>(log, f"{prev}c12 ");
    }));
}
"#;

type Received = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

fn bus_app() -> (TurTestApp, tur_engine::core::event_bus::EventBusHandle, u64) {
    let app = TurTestApp::new(300.0, 200.0).unwrap();
    app.load_rut_module(FIXTURE).unwrap();
    // Boot: register the handlers + mount the tree (the context token is
    // the boxed App — held for the late-bind entry).
    let cx = app.call_rut_entry_opaque("entry_start").unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let handle = app.app().event_bus_handle();
    (app, handle, cx)
}

fn bus_log(app: &TurTestApp) -> String {
    let id = app.query_element(&["bus", "log-text"]).unwrap();
    let id = ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<TextElement>()
            .map(|c| {
                c.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

#[test]
fn host_to_module_deliveries_isolation_and_silent_drop() {
    let (app, bus, _cx) = bus_app();

    // Single + multiple: every queued message delivers, in order.
    bus.emit_to_module(7, b"ping".to_vec());
    bus.emit_to_module(7, b"two".to_vec());
    bus.emit_to_module(7, b"three".to_vec());
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        bus_log(&app),
        "c7 c7 c7 ",
        "single + multiple deliver in order"
    );

    // Channel isolation: a message on N never fires handlers on other
    // channels.
    bus.emit_to_module(8, b"for-eight".to_vec());
    bus.emit_to_module(7, b"for-seven".to_vec());
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        bus_log(&app),
        "c7 c7 c7 c8 c7 ",
        "isolation: c8 fires only c8"
    );

    // Silent drop: a handler-less channel drops quietly and doesn't
    // disturb the others.
    bus.emit_to_module(99, b"nobody-home".to_vec());
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        bus_log(&app),
        "c7 c7 c7 c8 c7 ",
        "the handler-less drop is silent"
    );
}

#[test]
fn module_to_host_round_trip_and_host_handler_isolation() {
    let (app, bus, _cx) = bus_app();
    let received: Received = Arc::new(Mutex::new(Vec::new()));
    let received9 = received.clone();
    bus.on_bus_event(9, move |bytes| {
        received9.lock().unwrap().push((9, bytes));
    });
    let hits: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let hits9 = hits.clone();
    bus.on_bus_event(9, move |_| hits9.lock().unwrap().push(9));
    let hits10 = hits.clone();
    bus.on_bus_event(10, move |_| hits10.lock().unwrap().push(10));

    // Two forwards (channel 7) with a non-forwarding message between
    // (channel 8 — the module doesn't forward it; and no host handler on
    // any channel 8 handler rail fires).
    bus.emit_to_module(7, b"a".to_vec());
    bus.emit_to_module(8, b"not-forwarded".to_vec());
    bus.emit_to_module(7, b"round-trip-payload".to_vec());
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        received.lock().unwrap().clone(),
        vec![(9, b"a".to_vec()), (9, b"round-trip-payload".to_vec())],
        "byte fidelity: payloads round-trip host→module→host unchanged, in order"
    );
    assert_eq!(
        *hits.lock().unwrap(),
        vec![9, 9],
        "host handlers: channel isolation"
    );
    assert_eq!(bus_log(&app), "c7 c8 c7 ", "the log rail saw all three");
}

#[test]
fn late_registration_receives_the_next_message() {
    let (app, bus, cx) = bus_app();
    // Before the bind: a message on 12 drops silently.
    bus.emit_to_module(12, b"early".to_vec());
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(bus_log(&app), "", "no handlers yet — the drop is silent");

    // Bind channel 12 through the control entry, then the next message
    // delivers.
    app.call_rut_entry_cx_u64("late_bind", cx, 0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    bus.emit_to_module(12, b"late".to_vec());
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(bus_log(&app), "c12 ", "the late-bound handler receives");
}
