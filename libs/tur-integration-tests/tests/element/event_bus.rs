//! Integration tests for the event bus (`tur:std` `eventBus`) and its
//! interaction with `decodeUtf8` / `encodeUtf8`.
//!
//! Validates bidirectional byte-channel communication between the Rust host
//! and the JS realm:
//! - Host→JS: `EventBus::of(&app).emit_to_js(bytes)` → JS `on` callback
//! - JS→Host: JS `eventBus.send(bytes)` → host `on_bus_event` handler
//! - Multiple messages in both directions
//! - JSON payload with developer-managed id correlation (no built-in id —
//!   developers encode whatever structure they need inside the byte payload)

use std::rc::Rc;
use std::sync::Mutex;

use tur_engine::EventBus;
use tur_integration_tests::TurTestApp;

// ===========================================================================
// Host → JS
// ===========================================================================

#[test]
fn host_emits_bytes_js_decodes() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { eventBus, decodeUtf8 } from "tur:std";

        globalThis.__received = "";
        eventBus.on((payload) => {
            globalThis.__received = decodeUtf8(payload);
        });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus registered");
    bus.emit_to_js(b"hello from host".to_vec());

    app.wait_for(|a| !a.eval_js("globalThis.__received").is_empty());
    assert_eq!(app.eval_js("globalThis.__received"), "hello from host");
}

#[test]
fn host_emits_multiple_messages_js_collects_all() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { eventBus, decodeUtf8 } from "tur:std";

        globalThis.__messages = [];
        eventBus.on((payload) => {
            globalThis.__messages.push(decodeUtf8(payload));
        });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(b"first".to_vec());
    bus.emit_to_js(b"second".to_vec());
    bus.emit_to_js(b"third".to_vec());

    app.wait_for(|a| a.eval_js("globalThis.__messages.length") == "3");
    assert_eq!(
        app.eval_js(r#"globalThis.__messages.join(",")"#),
        "first,second,third"
    );
}

// ===========================================================================
// JS → Host
// ===========================================================================

#[test]
fn js_sends_bytes_host_receives() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    let received: Rc<Mutex<Vec<String>>> = Rc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.on_bus_event(move |bytes| {
        let text = String::from_utf8_lossy(&bytes).to_string();
        received_clone.lock().unwrap().push(text);
    });

    app.eval_module_source(
        r#"
        import { eventBus, encodeUtf8 } from "tur:std";

        eventBus.send(encodeUtf8("hello from JS"));
        "#,
    )
    .expect("module");

    app.wait_for(|_| !received.lock().unwrap().is_empty());

    let msgs = received.lock().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], "hello from JS");
}

#[test]
fn js_sends_raw_uint8array() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    let received: Rc<Mutex<Vec<Vec<u8>>>> = Rc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.on_bus_event(move |bytes| {
        received_clone.lock().unwrap().push(bytes);
    });

    // Send raw bytes without encodeUtf8 — verify the channel is a pure byte pipe
    app.eval_module_source(
        r#"
        import { eventBus } from "tur:std";

        eventBus.send(new Uint8Array([0, 128, 255, 1, 2, 3]));
        "#,
    )
    .expect("module");

    app.wait_for(|_| !received.lock().unwrap().is_empty());

    let msgs = received.lock().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], vec![0, 128, 255, 1, 2, 3]);
}

// ===========================================================================
// Bidirectional + JSON correlation
// ===========================================================================

#[test]
fn json_round_trip_with_id_correlation() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    // JS sends a JSON-encoded request → host echoes it back with a prefix →
    // JS parses the response. Validates the full byte round-trip:
    // JS → bytes → host → bytes → JS.
    let bus = EventBus::of(app.app()).expect("event bus");
    let bus_clone = bus.clone();
    bus.on_bus_event(move |bytes| {
        let mut echoed = b"echo:".to_vec();
        echoed.extend_from_slice(&bytes);
        bus_clone.emit_to_js(echoed);
    });

    app.eval_module_source(
        r#"
        import { eventBus, encodeUtf8, decodeUtf8 } from "tur:std";

        globalThis.__result = "";
        eventBus.on((payload) => {
            globalThis.__result = decodeUtf8(payload);
        });

        eventBus.send(encodeUtf8(JSON.stringify({ id: 42, value: "test" })));
        "#,
    )
    .expect("module");

    app.wait_for(|a| !a.eval_js("globalThis.__result").is_empty());
    let result = app.eval_js("globalThis.__result");
    assert!(
        result.contains("echo:"),
        "should contain echo prefix: {result}"
    );
    assert!(result.contains("\"id\":42"), "should contain id: {result}");
}

#[test]
fn host_emits_raw_binary_js_reads_bytes() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { eventBus } from "tur:std";

        globalThis.__byteLen = 0;
        globalThis.__first = -1;
        globalThis.__last = -1;
        eventBus.on((payload) => {
            globalThis.__byteLen = payload.byteLength;
            globalThis.__first = payload[0];
            globalThis.__last = payload[payload.byteLength - 1];
        });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(vec![10, 20, 30, 40, 50]);

    app.wait_for(|a| a.eval_js("globalThis.__byteLen") == "5");
    assert_eq!(app.eval_js("globalThis.__first"), "10");
    assert_eq!(app.eval_js("globalThis.__last"), "50");
}
