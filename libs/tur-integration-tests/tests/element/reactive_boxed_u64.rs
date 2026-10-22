//! The boxed rail's u64 lane — `source<u64>` / `ctx.get<u64>` through the
//! engine's number stamps. The KV holds ONE number variant, so the mint's
//! stamp (`rs_box_stamp`) picks the read's re-stamp width; the u64 lane
//! must round-trip a value beyond the i32 range losslessly and recover
//! through `unopaque<u64>` — a `TY_U64`-sealed box, never the i64 lane's
//! tag. (`rs_box_stamp` must read the box's TYPE TAG, not just its payload:
//! the VM carries both integer lanes as `Value::I64` payloads.)

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// Boot mints `source<u64>(3000000000)` and derives the label — the full
/// box lane in one breath: the mint decodes the box into the KV
/// (`boxed_to_value`), the derive's `ctx.get<u64>` re-seals it (`seal_box`
/// under the mint's stamp) and `unopaque<u64>` recovers the value.
const U64_LABEL_RUT: &str = r#"
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ DeriveCtx, Readable, Source, derive, source };
use tur_kit::text::core::{ Text };

entry fn start() {
    // The capture stays CONCRETE (`Source<u64>`, not `Readable<u64>`):
    // the known upstream interface-slot interning bug misbinds a fresh
    // `Readable<u64>` slot at boot ("no impl for interface slot N").
    let count: Source<u64> = source<u64>(3000000000u64);
    let label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"Count: {ctx.get<u64>(count)}";
    });
    let col = Column().child(Text().text(label).query_key("rut/u64").build());
    mount(col.build());
}
"#;

#[test]
fn the_boxed_u64_lane_roundtrips_large_values() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_rut_module(U64_LABEL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let id = app
        .query_element(&["rut", "u64"])
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
        text, "Count: 3000000000",
        "the u64 box lane round-trips the value through the u64 stamp"
    );
}
