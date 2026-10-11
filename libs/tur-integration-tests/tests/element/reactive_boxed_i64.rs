//! The boxed rail's i64 lane — `source<i64>` / `ctx.get<i64>` through the
//! engine's number stamps. The KV holds ONE number variant, so the mint's
//! stamp (`rs_box_stamp`) picks the read's re-stamp width; the i64 lane
//! must round-trip NEGATIVE values losslessly (the counter's -1 walk) and
//! recover through `unopaque<i64>` — a `TY_I64`-sealed box, never the u64
//! lane's tag.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// Boot mints `source<i64>(-5)` and derives the label — the full box lane
/// in one breath: the mint decodes the box into the KV (`boxed_to_value`),
/// the derive's `ctx.get<i64>` re-seals it (`seal_box`) and `unopaque<i64>`
/// recovers the sign.
const I64_LABEL_RUT: &str = r#"
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ DeriveCtx, Readable, Source, derive, source };
use tur_kit::text::core::{ Text };

entry fn start() {
    // The capture stays CONCRETE (`Source<i64>`, not `Readable<i64>`):
    // the known upstream interface-slot interning bug misbinds a fresh
    // `Readable<i64>` slot at boot ("no impl for interface slot N").
    let count: Source<i64> = source<i64>(-5);
    let label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"Count: {ctx.get<i64>(count)}";
    });
    let col = Column().child(Text().text_bound(label).query_key("rut/i64").build());
    mount(col.build());
}
"#;

#[test]
fn the_boxed_i64_lane_roundtrips_negative_values() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_rut_module(I64_LABEL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let id = app
        .query_element(&["rut", "i64"])
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
        text, "Count: -5",
        "the i64 box lane round-trips the negative sign"
    );
}
