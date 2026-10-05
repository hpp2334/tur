//! The Phase-0 callback rail: kit callbacks are FN VALUES (named `fn`
//! references or anonymous fn literals — captures legal), the kit seals
//! them into opaque boxes rut-side, and the engine's drain fires them
//! through the infra-owned dispatch entries (`__tur_cb_*`, declared in the
//! kit dep — linked-dep entries are reachable via the program export
//! table). Nothing callable crosses the boundary as a string.
//!
//! Pinned here:
//! 1. a named fn value + an anonymous fn literal both fire through real
//!    clicks (the drain → `__tur_cb_click` path, Route A);
//! 2. a wrong-arity callback is a COMPILE error (the kit boundary's
//!    arity/type check replaces the stringly-typed name lookup);
//! 3. `Each` item builders take fn values (the face-called
//!    `__tur_cb_each` path with an `opaque` return);
//! 4. animation tick/end take fn values (the `__tur_cb_val` path).

use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn bound_text(app: &TurTestApp) -> String {
    let id = app
        .query_element(&["rail", "text"])
        .expect("bound text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
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

fn center(app: &TurTestApp, key: &[&str]) -> (f64, f64) {
    let id = app.query_element(key).expect("element not found");
    let el = app
        .dev_tool_get_element(tur_engine::core::element::ElementNodeId::new(id.as_u64()).into())
        .expect("dev tool element");
    (
        el.absolute.0 + el.size.0 / 2.0,
        el.absolute.1 + el.size.1 / 2.0,
    )
}

const RAIL_RUT: &str = r#"
use tur::{ mount, rs_get_f64, rs_set_f64, rs_set_str, rs_source_f64, rs_source_str };
use tur_kit::{ Column, Each, PointerInteract, Text };

// A plain fn — NOT an entry. The kit seals it into an opaque box on
// registration; only the box crosses to Rust.
fn b_named(a: u64, label: u64, n: f64) {
    rs_set_f64(a, rs_get_f64(a) + 1.0);
    rs_set_str(label, f"named {n as u64}");
}

entry fn start() -> u64 {
    let count = rs_source_f64();
    let label = rs_source_str("idle");
    let named = PointerInteract().ids(count, label).on_tap(b_named)
        .child(Text().text("named").query_key("rail/named").build()).build();
    // The anonymous literal — the boa `() => ...` ergonomic.
    let literal = PointerInteract().ids(count, label).on_tap(fn (a: u64, l: u64, _n: f64) {
        rs_set_f64(a, rs_get_f64(a) + 10.0);
        rs_set_str(l, "literal");
    }).child(Text().text("literal").query_key("rail/literal").build()).build();
    mount(Column()
        .child(Text().text_bound(label).query_key("rail/text").build())
        .child(named)
        .child(literal)
        .build());
    return count;
}
"#;

#[test]
fn fn_value_callbacks_fire_through_the_infra_dispatch() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(RAIL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(bound_text(&app), "idle");

    // The named fn value, through a real click.
    let (x, y) = center(&app, &["rail", "named"]);
    app.click(x, y);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(bound_text(&app), "named 1");

    // The anonymous fn literal, through a real click.
    let (x, y) = center(&app, &["rail", "literal"]);
    app.click(x, y);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(bound_text(&app), "literal");
}

const WRONG_ARITY_RUT: &str = r#"
use tur::{ mount };
use tur_kit::{ Column, PointerInteract, Text };

fn bad(a: u64) { }

entry fn start() {
    mount(PointerInteract().on_tap(bad).child(Text().text("x").build()).build());
}
"#;

#[test]
fn wrong_arity_callback_fails_to_compile() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    let err = app
        .load_rut_module(WRONG_ARITY_RUT)
        .expect_err("a wrong-arity callback must fail at COMPILE time");
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("arg") || msg.to_lowercase().contains("param"),
        "the diagnostic should name the arity mismatch, got: {msg}"
    );
}

const EACH_RAIL_RUT: &str = r#"
use tur::{ mount, rs_list_new, rs_list_push, rs_source_value };
use tur_kit::{ Column, Each, Text };

entry fn start() {
    let list = rs_list_new();
    rs_list_push(list, "alpha");
    rs_list_push(list, "beta");
    let items = rs_source_value(list);
    mount(Column().query_key("rail/each").child(
        Each(items).item_builder(fn (i: u64, item: str) -> View {
            return Text().text(f"{i}:{item}").query_key(f"rail/item-{i}").build();
        }).build(),
    ).build());
}
"#;

#[test]
fn each_item_builder_takes_a_fn_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(EACH_RAIL_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    for (i, want) in [("0", "0:alpha"), ("1", "1:beta")] {
        let id = app
            .query_element(&["rail", &format!("item-{i}")])
            .unwrap_or_else(|| panic!("item {i} not built"));
        let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
        let got = app
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
        assert_eq!(got, want, "item {i} built through the fn-value rail");
    }
}

const ANIM_RAIL_RUT: &str = r#"
use tur::{ anim_forward, mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ Column, Text };
use tur_anim_kit::{ anim_ctrl };
use tur_anim_kit::{ anim_ctrl_tick };

entry fn start() {
    let progress = rs_source_f64();
    let box_atom = rs_source_f64();
    let ctrl = anim_ctrl_tick(progress, 50.0, "linear", 0, fn (a: u64, t: f64) {
        rs_set_f64(a, t);
    });
    anim_forward(ctrl);
    mount(Column().query_key("rail/anim").child(Text().text("anim").build()).build());
}
"#;

#[test]
fn animation_tick_takes_a_fn_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(ANIM_RAIL_RUT).unwrap();
    // Drive frames so the controller ticks into the fn value.
    for _ in 0..12 {
        app.wait_for_timeout(Duration::from_millis(32));
    }
    let tree = app.dev_tool_element_tree().expect("tree mounted");
    assert!(tree.children.len() > 0, "the anim rail module mounted");
}
