//! Click integrity — the cross-pump churn shapes the playground journey
//! exposed (intermittently swallowed taps on small `PointerInteract`
//! buttons; a late overlay receiving a release it never received the press
//! for). The press and the release are separate DOM events in a real
//! browser and land in separate worker pumps; between them the pressed
//! subtree may rebuild (hover swaps, async content) or an overlay may
//! appear over the press site. These tests pin the dispatch law:
//!
//! - a click survives an inner-subtree rebuild between down and up (the
//!   `PointerInteract` wrapper was pressed AND released over — its
//!   innermost child's identity is irrelevant), and
//! - an element that appeared after the press never receives the click.

use std::time::Duration;

use tur_engine::builtin_plugins::text::elements::TextElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

// click-integrity: a target PI whose inner face swaps shape (a Condition
// over the stage source) and a late overlay PI that can cover it. The
// stage machine: 0 = the plain face; 1 = the swapped face (subtree
// rebuild); 2 = the plain face + overlay; 3 = both. `start` answers the
// stage atom id; `set_stage` flips it (the corpus probe-entry pattern).
const SOURCE: &str = r#"
use tur_kit::control_flow::control::{ Condition };
use tur_kit::flags::{ Align };
use tur_kit::gesture::pointer::{ PointerInteract };
use tur_kit::handles::{ View, mount };
use tur_kit::layout::box::{ Container, SizedBox };
use tur_kit::layout::flex::{ Column };
use tur_kit::layout::stack::{ Stack };
use tur_kit::reactive::{ DeriveCtx, Mutation, MutationCtx, Readable, Source, bridge, derive, mutate, source, sv };
use tur_kit::text::core::{ Text };

fn face_text(t: str, c: u64) -> View {
    return Text().text(sv(t)).font_size(14.0).color(c).build();
}

entry fn start() -> u64 {
    let clicks: Source<f64> = source<f64>(0);
    let boom: Source<f64> = source<f64>(0);
    let stage: Source<f64> = source<f64>(0);
    let fired_label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"fired {ctx.get<f64>(clicks)}";
    });
    let boom_label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"boom {ctx.get<f64>(boom)}";
    });
    let swap_on: Readable<bool> = derive<bool>(fn (ctx: DeriveCtx) -> bool {
        let s = ctx.get<f64>(stage);
        return s == 1.0 || s == 3.0;
    });
    let overlay_on: Readable<bool> = derive<bool>(fn (ctx: DeriveCtx) -> bool {
        let s = ctx.get<f64>(stage);
        return s >= 2.0;
    });
    let b_click = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(clicks, ctx.get<f64>(clicks) + 1.0);
    });
    let b_boom = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(boom, ctx.get<f64>(boom) + 1.0);
    });
    let target = PointerInteract()
        .on_click(b_click)
        .query_key("target")
        .child(
            Container().width_height(300.0, 80.0).color(sv(0x6366F1FFu64)).alignment(Align::Center)
                .child(
                    Condition(swap_on)
                        .then_build(fn () -> View {
                            return Column().child(face_text("b-face", 0xFFFFFFFFu64)).build();
                        })
                        .else_build(fn () -> View {
                            return face_text("a-face", 0xFFFFFFFFu64);
                        })
                        .build(),
                )
                .build(),
        )
        .build();
    let overlay = Condition(overlay_on)
        .then_build(fn () -> View {
            return PointerInteract()
                .on_click(b_boom)
                .query_key("overlay")
                .child(Container().width_height(300.0, 80.0).color(sv(0xEF444470u64)).build())
                .build();
        })
        .else_build(fn () -> View {
            return Container().width_height(300.0, 80.0).build();
        })
        .build();
    let stack = Stack().child(target).child(overlay).build();
    let page = Column()
        .child(Text().text(fired_label).query_key("fired").font_size(12.0).color(0x1E293BFFu64).build())
        .child(Text().text(boom_label).query_key("boom").font_size(12.0).color(0x1E293BFFu64).build())
        .child(SizedBox(0.0, 8.0).build())
        .child(stack)
        .build();
    mount(Container().color(sv(0xFFFFFFFFu64)).child(page).build());
    return stage.atom_id();
}

entry fn set_stage(atom: u64, v: f64) {
    let stage = Source<f64>::of(bridge(), atom, false, 1);
    MutationCtx::over(bridge()).set<f64>(stage, v);
}
"#;

fn boot() -> (TurTestApp, u64) {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(SOURCE).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let stage = app.rut_start_answer();
    (app, stage)
}

fn span_text(app: &TurTestApp, key: &[&str]) -> String {
    let id = app.query_element(key).unwrap();
    app.with_element(ElementNodeId::new(id.as_u64()), |e| {
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

fn target_center(app: &TurTestApp) -> (f64, f64) {
    let id = app.query_element(&["target"]).unwrap();
    app.get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .center()
}

/// The press site's innermost subtree was replaced between down and up
/// (the deepest element at press time no longer exists at release). The
/// click must still fire on the `PointerInteract` that was both pressed
/// and released over — the press-and-release pair, not the leaf's
/// identity, is the click's subject.
#[test]
fn click_survives_inner_subtree_rebuild_between_down_and_up() {
    let (mut app, stage) = boot();
    let (cx, cy) = target_center(&app);

    app.mouse_down(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    // The face swap: the a-face leaf (the press's deepest hit) is destroyed
    // and a b-face subtree takes its place.
    app.call_rut_entry("set_stage", stage, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app.mouse_up(cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        span_text(&app, &["fired"]),
        "fired 1",
        "the click must survive the inner-subtree rebuild between down and up"
    );
}

/// An overlay that appeared AFTER the press never receives the release's
/// click — it was not part of the press, so it is not part of the click
/// (the late-overlay misfire class: a scrim/panel appearing mid-press).
#[test]
fn release_over_late_overlay_does_not_fire_overlay() {
    let (mut app, stage) = boot();
    let (cx, cy) = target_center(&app);

    app.mouse_down(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    app.call_rut_entry("set_stage", stage, 2.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app.mouse_up(cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(
        span_text(&app, &["boom"]),
        "boom 0",
        "an overlay that appeared after the press must not receive the click"
    );
}

/// Control: the plain same-pump click still fires exactly once.
#[test]
fn plain_click_fires_once() {
    let (mut app, _stage) = boot();
    let (cx, cy) = target_center(&app);

    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(span_text(&app, &["fired"]), "fired 1");

    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(span_text(&app, &["fired"]), "fired 2");
    assert_eq!(span_text(&app, &["boom"]), "boom 0");
}

/// Control: a release far outside the press site fires nothing (the
/// intersection law must not loosen the drag-release gate).
#[test]
fn release_far_from_press_fires_nothing() {
    let (mut app, _stage) = boot();
    let (cx, cy) = target_center(&app);

    app.mouse_down(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
    // Well outside the 300x80 target (a drag-release onto bare page).
    app.mouse_up(cx + 350.0, cy + 200.0);
    app.wait_for_timeout(Duration::ZERO);

    assert_eq!(span_text(&app, &["fired"]), "fired 0");
    assert_eq!(span_text(&app, &["boom"]), "boom 0");
}
