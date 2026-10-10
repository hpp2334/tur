//! The callback rail: HANDLERS are mutations over the `MutationCtx` (the
//! M2 surface — `on_click(mutate(...))`); the branch builders (`Each`'s
//! item_builder) stay FN VALUES (named `fn` references or anonymous fn
//! literals — captures legal), the kit seals them into opaque boxes
//! rut-side, and the engine's drain / face fires them through the
//! infra-owned dispatch entries (`__tur_cb_*`). Nothing callable crosses
//! the boundary as a string.
//!
//! Pinned here:
//! 1. a named mutation + an anonymous mutation literal both fire through
//!    real clicks (the gesture dispatch enqueues; the flush's mutation
//!    pass invokes with the ctx);
//! 2. a wrong-shaped handler is a COMPILE error (the kit boundary's
//!    arity/type check replaces the stringly-typed name lookup);
//! 3. `Each` item builders take fn values (the face-called
//!    `__tur_cb_each` path with an `opaque` return);
//! 4. animation ticks take mutations (the `__tur_cb_mf64` path).

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
use tur_kit::gesture::pointer::{ PointerInteract };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_kit::text::core::{ Text };

entry fn start() -> u64 {
    let count: Readable<f64> = source<f64>(0.0);
    let label: Readable<str> = source<str>("idle");
    // The named mutation — the kit seals it into an opaque box on
    // registration; only the box crosses to Rust.
    let b_named = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(count, ctx.get<f64>(count) + 1.0);
        ctx.set<str>(label, f"named {ctx.get<f64>(count) as u64}");
    });
    let named = PointerInteract().on_click(b_named)
        .child(Text().text("named").query_key("rail/named").build()).build();
    // The anonymous mutation literal — the boa `mutate(() => ...)` twin.
    let literal = PointerInteract().on_click(mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(count, ctx.get<f64>(count) + 10.0);
        ctx.set<str>(label, "literal");
    })).child(Text().text("literal").query_key("rail/literal").build()).build();
    mount(Column()
        .child(Text().text_bound(label).query_key("rail/text").build())
        .child(named)
        .child(literal)
        .build());
    return count.atom_id();
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
use tur_kit::gesture::pointer::{ PointerInteract };
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_kit::text::core::{ Text };

entry fn start() {
    // A nil-arg mutation handed to the typed-arg pad — the shapes
    // disagree (the kit boundary's check rejects it at compile time).
    let m = mutate(fn (ctx: MutationCtx, _e: nil) {
        let _ = ctx;
    });
    mount(PointerInteract().on_pointer_down(m).child(Text().text("x").build()).build());
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
use tur_kit::control_flow::control::{ Each };
use tur_kit::handles::{ ListHandle, View, list_new, mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ Source, source };
use tur_kit::text::core::{ Text };

entry fn start() {
    let list: ListHandle = list_new();
    list.push("alpha");
    list.push("beta");
    let items = source<opaque>(list.raw());
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

use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };
use tur_kit::reactive::{ Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_kit::text::core::{ Text };
use tur_anim_kit::{ anim_ctrl_tick };

entry fn start() {
    let progress: Readable<f64> = source<f64>(0.0);
    let box_r: Readable<f64> = source<f64>(0.0);
    // The tick is a MUTATION over the ctx (the eased value is the
    // invocation payload; the channel rides by capture).
    let a_tick: ?Mutation<f64> = mutate<f64>(fn (ctx: MutationCtx, t: f64) {
        ctx.set<f64>(box_r, t);
        let _ = progress;
    });
    let ctrl = anim_ctrl_tick(50.0, "linear", 0, a_tick);
    ctrl.forward();
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
