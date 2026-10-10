//! Integration tests for the bytes helpers — `encode_utf8` / `decode_utf8`
//! (the `tur_host` pkg rows). The canonical string↔bytes round-trip: net
//! bodies cross as raw bytes and decode through these rows.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// Round-trip ASCII + Unicode through both rows, reading the answer back
/// through a bound label (the rut corpus's standard probe).
const ENCODE_RUT: &str = r#"
use tur_host::{ decode_utf8, encode_utf8 };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ Source, source };
use tur_kit::text::core::{ Text };

entry fn start() {
    let ascii = decode_utf8(encode_utf8("hello world"));
    let unicode = decode_utf8(encode_utf8("héllo 世界 🚀"));
    let empty = decode_utf8(encode_utf8(""));
    // The mint seeds the transcript (a boot write with no entry-rail ctx
    // is a construction-time value, not a state transition).
    let label: Source<str> = source<str>(f"{ascii}|{unicode}|{empty}|");

    let col = Column().child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
}
"#;

#[test]
fn encode_decode_roundtrips_through_rut_rows() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_rut_module(ENCODE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let id = app
        .query_element(&["rut", "text"])
        .expect("bound label not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    let text = app
        .with_element(id, |e| {
            e.cast::<tur_engine::builtin_plugins::text::TextElement>()
                .map(|c| {
                    c.spans()
                        .iter()
                        .map(|s| s.text.as_str())
                        .collect::<String>()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    assert_eq!(
        text, "hello world|héllo 世界 🚀||",
        "both rows round-tripped; the empty string round-trips to empty"
    );
}
