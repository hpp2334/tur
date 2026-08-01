//! Integration tests for `decodeUtf8` / `encodeUtf8` (merged into `tur:std`).
//!
//! boa does not implement `TextDecoder` / `TextEncoder`, so these engine-side
//! natives are the canonical way to round-trip between JS strings and
//! `Uint8Array`.

use tur_integration_tests::TurTestApp;

#[test]
fn encode_decode_roundtrip_ascii() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { encodeUtf8, decodeUtf8 } from "tur:std";

        const bytes = encodeUtf8("hello world");
        globalThis.__isUint8Array = bytes instanceof Uint8Array;
        globalThis.__len = bytes.byteLength;
        globalThis.__decoded = decodeUtf8(bytes);
        "#,
    )
    .expect("module");

    assert_eq!(app.eval_js("globalThis.__isUint8Array"), "true");
    assert_eq!(app.eval_js("globalThis.__len"), "11");
    assert_eq!(app.eval_js("globalThis.__decoded"), "hello world");
}

#[test]
fn encode_decode_roundtrip_unicode() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { encodeUtf8, decodeUtf8 } from "tur:std";

        const text = "héllo 世界 🚀";
        const bytes = encodeUtf8(text);
        globalThis.__decoded = decodeUtf8(bytes);
        "#,
    )
    .expect("module");

    assert_eq!(
        app.eval_js("globalThis.__decoded"),
        // boa escapes non-ASCII in to_std_string_escaped
        "héllo 世界 🚀",
    );
}

#[test]
fn decode_arraybuffer() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { decodeUtf8 } from "tur:std";

        const ab = new ArrayBuffer(5);
        const view = new Uint8Array(ab);
        view[0] = 104; // h
        view[1] = 105; // i
        view[2] = 33;  // !
        view[3] = 10;  // \n
        view[4] = 63;  // ?
        globalThis.__decoded = decodeUtf8(ab);
        "#,
    )
    .expect("module");

    assert_eq!(app.eval_js("globalThis.__decoded"), "hi!\n?");
}

#[test]
fn encode_empty_string() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { encodeUtf8, decodeUtf8 } from "tur:std";

        const bytes = encodeUtf8("");
        globalThis.__len = bytes.byteLength;
        globalThis.__decoded = decodeUtf8(bytes);
        "#,
    )
    .expect("module");

    assert_eq!(app.eval_js("globalThis.__len"), "0");
    assert_eq!(app.eval_js("globalThis.__decoded"), "");
}

#[test]
fn decode_invalid_utf8_throws() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();

    app.eval_module_source(
        r#"
        import { decodeUtf8 } from "tur:std";

        try {
            // 0xFF is invalid as a UTF-8 start byte
            const bad = new Uint8Array([0xFF, 0xFE]);
            decodeUtf8(bad);
            globalThis.__threw = "no";
        } catch (e) {
            globalThis.__threw = "yes";
        }
        "#,
    )
    .expect("module");

    assert_eq!(app.eval_js("globalThis.__threw"), "yes");
}
