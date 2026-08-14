//! Integration tests for the event bus (`tur:std` `eventBus`) and its
//! interaction with `decodeUtf8` / `encodeUtf8`.
//!
//! Validates bidirectional multiplexed byte-channel communication between
//! the Rust host and the JS realm, keyed by `channel_id`:
//! - Host→JS: `EventBus::of(&app).emit_to_js(channel_id, bytes)` → JS `on`
//!   callback registered on `channel_id`
//! - JS→Host: JS `eventBus.send(channel_id, bytes)` → host `on_bus_event`
//!   handler registered on `channel_id`
//! - Multiple messages in both directions
//! - Channel isolation: a message on channel N only fires handlers on N
//! - JSON payload with developer-managed id correlation (no built-in id —
//!   developers encode whatever structure they need inside the byte payload)

use std::sync::{Arc, Mutex};

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
        eventBus.on(0, (payload) => {
            globalThis.__received = decodeUtf8(payload);
        });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus registered");
    bus.emit_to_js(0, b"hello from host".to_vec());

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
        eventBus.on(0, (payload) => {
            globalThis.__messages.push(decodeUtf8(payload));
        });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(0, b"first".to_vec());
    bus.emit_to_js(0, b"second".to_vec());
    bus.emit_to_js(0, b"third".to_vec());

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

    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.on_bus_event(0, move |bytes| {
        let text = String::from_utf8_lossy(&bytes).to_string();
        received_clone.lock().unwrap().push(text);
    });

    app.eval_module_source(
        r#"
        import { eventBus, encodeUtf8 } from "tur:std";

        eventBus.send(0, encodeUtf8("hello from JS"));
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

    let received: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.on_bus_event(0, move |bytes| {
        received_clone.lock().unwrap().push(bytes);
    });

    // Send raw bytes without encodeUtf8 — verify the channel is a pure byte pipe
    app.eval_module_source(
        r#"
        import { eventBus } from "tur:std";

        eventBus.send(0, new Uint8Array([0, 128, 255, 1, 2, 3]));
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
    bus.on_bus_event(0, move |bytes| {
        let mut echoed = b"echo:".to_vec();
        echoed.extend_from_slice(&bytes);
        bus_clone.emit_to_js(0, echoed);
    });

    app.eval_module_source(
        r#"
        import { eventBus, encodeUtf8, decodeUtf8 } from "tur:std";

        globalThis.__result = "";
        eventBus.on(0, (payload) => {
            globalThis.__result = decodeUtf8(payload);
        });

        eventBus.send(0, encodeUtf8(JSON.stringify({ id: 42, value: "test" })));
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
        eventBus.on(0, (payload) => {
            globalThis.__byteLen = payload.byteLength;
            globalThis.__first = payload[0];
            globalThis.__last = payload[payload.byteLength - 1];
        });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(0, vec![10, 20, 30, 40, 50]);

    app.wait_for(|a| a.eval_js("globalThis.__byteLen") == "5");
    assert_eq!(app.eval_js("globalThis.__first"), "10");
    assert_eq!(app.eval_js("globalThis.__last"), "50");
}

// ===========================================================================
// Channel isolation (multiplexing)
// ===========================================================================

#[test]
fn host_emits_to_isolated_channel_js() {
    // Register JS handlers on channels 1 and 2; emit to channel 1 only;
    // assert only the channel-1 handler fires.
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { eventBus, decodeUtf8 } from "tur:std";

        globalThis.__ch1 = "";
        globalThis.__ch2 = "";
        eventBus.on(1, (payload) => { globalThis.__ch1 = decodeUtf8(payload); });
        eventBus.on(2, (payload) => { globalThis.__ch2 = decodeUtf8(payload); });
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(1, b"ping-one".to_vec());

    app.wait_for(|a| !a.eval_js("globalThis.__ch1").is_empty());
    assert_eq!(app.eval_js("globalThis.__ch1"), "ping-one");
    assert_eq!(app.eval_js("globalThis.__ch2"), "");
}

#[test]
fn js_sends_to_isolated_channel_host() {
    // Register host handlers on channels 1 and 2; JS sends on channel 2
    // only; assert only the channel-2 handler fires.
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    let ch1: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let ch2: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let ch1_clone = ch1.clone();
    let ch2_clone = ch2.clone();

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.on_bus_event(1, move |bytes| {
        ch1_clone
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(&bytes).to_string());
    });
    bus.on_bus_event(2, move |bytes| {
        ch2_clone
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(&bytes).to_string());
    });

    app.eval_module_source(
        r#"
        import { eventBus, encodeUtf8 } from "tur:std";

        eventBus.send(2, encodeUtf8("only-channel-two"));
        "#,
    )
    .expect("module");

    app.wait_for(|_| !ch2.lock().unwrap().is_empty());

    assert!(ch1.lock().unwrap().is_empty(), "channel 1 must be empty");
    let ch2_msgs = ch2.lock().unwrap();
    assert_eq!(ch2_msgs.len(), 1);
    assert_eq!(ch2_msgs[0], "only-channel-two");
}

#[test]
fn no_handler_on_channel_no_panic() {
    // Emit/send on channels with no registered handlers — the bus must
    // silently drop (standard pub/sub) and never panic.
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { eventBus, encodeUtf8 } from "tur:std";

        globalThis.__delivered = false;
        eventBus.on(5, () => { globalThis.__delivered = true; });
        // Send on an unregistered channel (9) — should be a no-op.
        eventBus.send(9, encodeUtf8("nobody-listens"));
        "#,
    )
    .expect("module");

    let bus = EventBus::of(app.app()).expect("event bus");
    // Emit on an unregistered channel (7) — should be a no-op.
    bus.emit_to_js(7, b"nobody-listens-either".to_vec());

    // Pump a frame so the subsystem drains both queues. The channel-5
    // handler must NOT fire (no one emitted on 5).
    app.wait_for_timeout(std::time::Duration::ZERO);
    assert_eq!(app.eval_js("globalThis.__delivered"), "false");
}
