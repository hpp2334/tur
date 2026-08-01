//! Integration tests for the event bus (`tur:std` `eventBus`).
//!
//! Validates bidirectional byte-channel communication between the Rust host
//! and the JS realm, including:
//! - Host→JS: `EventBus::of(&app).emit_to_js(bytes)` → JS `on` callback
//! - JS→Host: JS `eventBus.send(bytes)` → host `on_bus_event` handler
//! - Round-trip with developer-encoded JSON payload (using `tur:encode`)

use std::rc::Rc;
use std::sync::Mutex;

use tur_engine::EventBus;
use tur_integration_tests::TurTestApp;

#[test]
fn host_emits_js_receives() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { eventBus } from "tur:std";
        import { decodeUtf8 } from "tur:encode";

        globalThis.__received = "";
        eventBus.on((payload) => {
            globalThis.__received = decodeUtf8(payload);
        });
        "#,
    )
    .expect("module");

    // Host sends bytes to JS
    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(b"hello from host".to_vec());

    app.wait_for(|a| !a.eval_js("globalThis.__received").is_empty());
    assert_eq!(app.eval_js("globalThis.__received"), "hello from host");
}

#[test]
fn js_sends_host_receives() {
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
        import { eventBus } from "tur:std";
        import { encodeUtf8 } from "tur:encode";

        eventBus.send(encodeUtf8("hello from JS"));
        "#,
    )
    .expect("module");

    app.settle();

    let msgs = received.lock().unwrap();
    assert_eq!(msgs.len(), 1, "host should have received one message");
    assert_eq!(msgs[0], "hello from JS");
}

#[test]
fn multiple_messages_both_directions() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    let host_received: Rc<Mutex<Vec<u32>>> = Rc::new(Mutex::new(Vec::new()));
    let host_received_clone = host_received.clone();

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.on_bus_event(move |bytes| {
        let text = String::from_utf8_lossy(&bytes).to_string();
        if let Ok(n) = text.parse::<u32>() {
            host_received_clone.lock().unwrap().push(n);
        }
    });

    app.eval_module_source(
        r#"
        import { eventBus } from "tur:std";
        import { encodeUtf8, decodeUtf8 } from "tur:encode";

        let count = 0;
        eventBus.on((payload) => {
            count = Number(decodeUtf8(payload));
            count++;
            eventBus.send(encodeUtf8(String(count)));
        });

        // Kick off: send 0 to JS
        // (host will send the first message below)
        "#,
    )
    .expect("module");

    // Host sends "0" → JS increments to 1 → sends "1" back → host receives
    bus.emit_to_js(b"0".to_vec());
    app.wait_for(|a| host_received.lock().unwrap().len() >= 1);

    let msgs = host_received.lock().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], 1);
}
