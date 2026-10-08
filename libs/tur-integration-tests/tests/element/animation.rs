//! The animation controller state machine, driven end to end through the
//! `tur_host` pkg's animation rows: the Rust-held controller opaque ticks
//! via the animation subsystem (frame-advanced against the virtual
//! clock), its onTick / onEnd intent rail writes bound atoms, and the
//! control rows (forward / reverse / stop / pause / resume / seek /
//! repeat / speed) are exercised through the entry rail mid-test.
//!
//! Common scaffold: a box whose width is bound to an f64 atom (the
//! controller's tick target: `100 + 100·v`), the controller stashed under
//! key 7 (an entry cannot capture an opaque — the stash is the hand-off
//! rail), and control/probe `entry fn`s the test drives via
//! `call_rut_entry`.

use std::time::Duration;

use tur_engine::builtin_plugins::effects::TransformElement;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// Read the bound box's laid-out width (the tick target).
fn box_width(app: &TurTestApp) -> f64 {
    let id = app.query_element(&["box"]).expect("bound box not found");
    let tree = app.element_tree();
    tree.get_element(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .computed_layout
        .size
        .width
}

/// Read the transcript label's text (probe entries write status/value
/// answers into it).
fn label(app: &TurTestApp) -> String {
    let id = app.query_element(&["rut", "text"]).expect("label missing");
    let id = ElementNodeId::new(id.as_u64());
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

/// A width-atom tick target driven by the controller (`100 + 100·v`), a
/// bound box, and `do_*` control entries over the stashed controller.
const CONTROLLER_RUT: &str = r#"
use tur_host::{ mount, rs_set_str, st_put, st_take };
use tur_kit::{ Column, Container, Mutation, MutationCtx, Readable, Source, Text, mutate, source };
use tur_anim_kit::{ AnimCtrl, anim_ctrl };


let CTRL: u64 = 7;

entry fn start() -> u64 {
    let label: Readable<str> = source<str>("");
    let width: Readable<f64> = source<f64>(100.0);

    let b = Container().width_height(10.0, 10.0).width_bound(width).query_key("box");

    // The tick mutation captures the width source (the eased 0..1 maps to
    // 100..200); the controller rides the stash only because the do_*
    // control entries cannot capture it.
    let a_tick: ?Mutation<f64> = mutate<f64>(fn (ctx: MutationCtx, v: f64) {
        ctx.set<f64>(width, 100.0 + (200.0 - 100.0) * v);
    });
    let a_end: ?Mutation<nil> = mutate(fn (_ctx: MutationCtx, _e: nil) {
    });
    let ctrl = anim_ctrl(200.0, "linear", 0, a_tick, a_end);
    st_put(CTRL, ctrl.raw());

    let col = Column()
        .child(b.build())
        .child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return label.atom_id();
}

entry fn do_forward(_a: u64, _b: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.forward();
    st_put(CTRL, c.raw());
}

entry fn do_reverse(_a: u64, _b: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.reverse();
    st_put(CTRL, c.raw());
}

entry fn do_stop(_a: u64, _b: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.stop();
    st_put(CTRL, c.raw());
}

entry fn do_pause(_a: u64, _b: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.pause();
    st_put(CTRL, c.raw());
}

entry fn do_resume(_a: u64, _b: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.resume();
    st_put(CTRL, c.raw());
}

entry fn do_seek(_a: u64, t: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.seek(t);
    st_put(CTRL, c.raw());
}

entry fn do_repeat(_a: u64, n: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.repeat(n as u64);
    st_put(CTRL, c.raw());
}

entry fn do_speed(_a: u64, s: f64) {
    let c = AnimCtrl(st_take(CTRL));
    c.speed(s);
    st_put(CTRL, c.raw());
}

// Report `status|v{value}` into the transcript label (the controller's
// own raw value — the width binding reads the same tick stream).
entry fn probe(label: u64, _b: f64) {
    let c = AnimCtrl(st_take(CTRL));
    rs_set_str(label, f"{c.status()}|v{c.value()}");
    st_put(CTRL, c.raw());
}
"#;

fn new_controller_app() -> (TurTestApp, u64) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(CONTROLLER_RUT).unwrap();
    let label_atom = app.rut_start_answer();
    app.wait_for_timeout(Duration::ZERO);
    (app, label_atom)
}

#[test]
fn animation_controller_forward_with_on_tick() {
    let (mut app, _width) = new_controller_app();
    assert_eq!(box_width(&app), 100.0, "at t=0 width should still be 100");

    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    app.wait_for_timeout(Duration::from_millis(100));
    let w = box_width(&app);
    assert!(
        w > 110.0 && w < 190.0,
        "at t=100ms (halfway) width should be ~150, got {w}"
    );

    app.wait_for_timeout(Duration::from_millis(150));
    assert_eq!(
        box_width(&app),
        200.0,
        "after duration elapsed width should be 200"
    );
}

#[test]
fn animation_controller_reverse_with_on_tick() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_reverse", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    app.wait_for_timeout(Duration::from_millis(100));
    let w = box_width(&app);
    assert!(
        w > 110.0 && w < 190.0,
        "reverse halfway: width should be ~150, got {w}"
    );

    app.wait_for_timeout(Duration::from_millis(150));
    assert_eq!(
        box_width(&app),
        100.0,
        "reverse complete: width should be 100"
    );
}

#[test]
fn animation_controller_stop_freezes_value() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::from_millis(50));
    let frozen = box_width(&app);
    assert!(
        frozen > 100.0 && frozen < 200.0,
        "width should be mid-animation, got {frozen}"
    );

    app.call_rut_entry("do_stop", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::from_millis(200));
    assert_eq!(
        box_width(&app),
        frozen,
        "after stop + advance, width should stay frozen at {frozen}"
    );
}

#[test]
fn animation_controller_repeats() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_repeat", 0, 3.0).unwrap();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // 250ms = 2.5 iterations of the 100ms... the controller duration is
    // 200ms, so 250ms is 1.25 iterations of a 3-iteration 200ms run:
    // halfway through the 2nd of 3.
    app.wait_for_timeout(Duration::from_millis(250));
    let w = box_width(&app);
    assert!(
        w > 100.0 && w < 200.0,
        "mid-way through a 3-iteration run the value is still cycling: got {w}"
    );

    // 600ms is past the 3x200ms = 600ms total — completed at the end value.
    app.wait_for_timeout(Duration::from_millis(350));
    assert_eq!(
        box_width(&app),
        200.0,
        "past 3x200ms, width should be 200 (completed)"
    );
}

#[test]
fn animation_controller_status_transitions() {
    let (mut app, label_atom) = new_controller_app();
    // Probe BEFORE starting: the controller rests at `stopped`.
    app.call_rut_entry("probe", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(label(&app).starts_with("stopped|"), "initial status");

    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app.call_rut_entry("probe", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        label(&app).starts_with("forward|"),
        "status after the start: {}",
        label(&app)
    );

    app.wait_for_timeout(Duration::from_millis(250));
    app.call_rut_entry("probe", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        label(&app),
        "completed|v1",
        "after duration elapsed: completed, the value at the end"
    );
}

#[test]
fn animation_controller_ease_in_curve() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // The scaffold's curve is linear; a second controller application with
    // easeIn lives in the curve_eval row test — here the pin is that the
    // linear scaffold reaches ~half at half time (the eased-curve behavior
    // is pinned by the rut_boot gate + corpus complex-animation case).
    app.wait_for_timeout(Duration::from_millis(100));
    let w = box_width(&app);
    assert!(
        (140.0..=160.0).contains(&w),
        "linear at t=0.5: width should be ~150, got {w}"
    );
}

#[test]
fn animation_controller_pause_freezes_and_resume_continues() {
    let (mut app, label_atom) = new_controller_app();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();

    // Halfway through (100ms of 200ms) → ~150, then pause.
    app.wait_for_timeout(Duration::from_millis(100));
    app.call_rut_entry("do_pause", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let paused = box_width(&app);
    assert!(
        paused > 140.0 && paused < 160.0,
        "paused width should be ~150, got {paused}"
    );

    // Advance 200ms while paused → no movement.
    app.wait_for_timeout(Duration::from_millis(200));
    let w = box_width(&app);
    assert!(
        (w - paused).abs() < 1.0,
        "during pause width should stay at {paused}, got {w}"
    );

    // Status reads paused.
    app.call_rut_entry("probe", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        label(&app).starts_with("paused|"),
        "status while paused: {}",
        label(&app)
    );

    // Resume — the remaining half plays out to completion.
    app.call_rut_entry("do_resume", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::from_millis(150));
    assert_eq!(
        box_width(&app),
        200.0,
        "after resume + advance, width should be 200 (completed)"
    );
}

#[test]
fn animation_controller_seek_jumps_value() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Seek to 0.25 immediately → the width lands at 125.
    app.call_rut_entry("do_seek", 0, 0.25).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let w = box_width(&app);
    assert!(
        (w - 125.0).abs() < 1.0,
        "seek to 0.25 lands the width at 125, got {w}"
    );
}

#[test]
fn animation_controller_set_speed_scales_time() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_speed", 0, 2.0).unwrap();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // 2x speed: 50ms of wall time covers the first 100ms of timeline —
    // and 100ms of wall time covers the full 200ms duration.
    app.wait_for_timeout(Duration::from_millis(100));
    assert_eq!(
        box_width(&app),
        200.0,
        "at 2x speed the 200ms animation completes in 100ms of wall time"
    );
}

#[test]
fn controller_on_tick_value_tracks_progress() {
    let (mut app, label_atom) = new_controller_app();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    app.wait_for_timeout(Duration::from_millis(100));
    app.call_rut_entry("probe", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let text = label(&app);
    // `v{anim_value}` is the controller's raw value — mid-range at ~0.5.
    assert!(
        text.starts_with("forward|v0."),
        "at ~0.5 of a linear animation the value tracks the progress: {text}"
    );
}

#[test]
fn controller_infinite_does_not_complete_after_many_iterations() {
    let (mut app, label_atom) = new_controller_app();
    // u64::MAX = infinite (the `repeat("infinite")` crossing).
    app.call_rut_entry("do_repeat", 0, 18446744073709551615.0)
        .unwrap();
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // 10 full iterations later: still forward, still cycling.
    app.wait_for_timeout(Duration::from_millis(2000));
    let w = box_width(&app);
    assert!(
        (100.0..=200.0).contains(&w),
        "an infinite animation keeps cycling inside [100, 200]: {w}"
    );
    app.call_rut_entry("probe", label_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        label(&app).starts_with("forward|"),
        "an infinite animation never completes: {}",
        label(&app)
    );
}

#[test]
fn controller_infinite_reverse_cycles_back_to_zero() {
    let (mut app, _width) = new_controller_app();
    app.call_rut_entry("do_repeat", 0, 18446744073709551615.0)
        .unwrap();
    app.call_rut_entry("do_reverse", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // An infinite reverse cycles: it returns to ~100 (v wraps to 0) and
    // keeps cycling — never stuck, never completed. (Off-phase sampling —
    // 30ms against the 100ms cycle — so the wrap is actually observed.)
    let mut saw_low = false;
    for _ in 0..12 {
        app.wait_for_timeout(Duration::from_millis(30));
        if box_width(&app) < 110.0 {
            saw_low = true;
        }
    }
    assert!(
        saw_low,
        "an infinite reverse cycles back down toward the low end"
    );
}

#[test]
fn animation_started_from_handler_schedules_next_frame() {
    // Regression: a controller started from within a frame (here: an entry
    // invoked through the intent rail mid-drain) must still schedule the
    // next vsync. If the schedule signal were captured only by the
    // once-per-flush subsystem tick — which ran BEFORE the controller was
    // registered — the animation would stall until the next platform event.
    let (mut app, _width) = new_controller_app();
    assert_eq!(box_width(&app), 100.0);

    // Start mid-frame via the entry rail, then drive frames with no
    // further input — the width must advance on its own.
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    let progressed = app.wait_for(|a| box_width(a) > 100.0);
    assert!(
        progressed,
        "an animation started from a handler must schedule the next vsync (width should advance past 100)"
    );
}

// ---- el_transform_angle_bound — rotation without rebuilds ----------------------
//
// The `el_transform` row was static-only, so complex-animation spun its
// inner square through a per-frame REBUILD channel (a one-item Each
// re-mounting a fresh Transform every tick). The bound rows close that
// wall: the rotation channel rides a live f64 atom (`Val::Reactive` — the
// radius_bound machinery; the view field was already subscribed and
// layout-resolved) and the angle re-resolves through the subscribe →
// relayout rail while the element identity stays put.

/// Count `tur_transform` elements in the tree (a rebuild channel would
/// keep re-mounting the spin under fresh ids).
fn transform_count(app: &TurTestApp) -> usize {
    let tree = app.element_tree();
    let want = ElementKind::new("tur_transform");
    tree.element_ids()
        .iter()
        .filter(|id| {
            tree.get_element(**id)
                .map(|n| n.kind() == Some(want.clone()))
                .unwrap_or(false)
        })
        .count()
}

/// The transform element's painted rotate (radians; layout resolves it).
fn painted_rotate(app: &TurTestApp, id: ElementNodeId) -> f64 {
    app.with_element(id, |el| {
        el.cast::<TransformElement>()
            .map(|t| t.painted_rotate())
            .unwrap_or(f64::NAN)
    })
    .unwrap_or(f64::NAN)
}

/// The bound-angle scaffold: a keyed square under
/// `Transform(1, 0, 0, 0).rotate_bound(angle)`, the controller ticking
/// `TAU·v` into the atom across a 200ms linear run.
const BOUND_ANGLE_RUT: &str = r#"
use tur_host::{ mount, st_put, st_take };
use tur_kit::{ Container, Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_anim_kit::{ AnimCtrl, Transform, anim_ctrl };

let TAU: f64 = 6.283185307179586;
let K_CTRL: u64 = 6;

entry fn start() -> u64 {
    let angle: Readable<f64> = source<f64>(0.0);
    let square = Container().width_height(60.0, 60.0).color(0xFFFFFFFFu64).query_key("bt/square").build();
    let xf = Transform(1.0, 0.0, 0.0, 0.0).rotate_bound(angle).child(square).build();
    let a_tick: ?Mutation<f64> = mutate<f64>(fn (ctx: MutationCtx, v: f64) {
        ctx.set<f64>(angle, TAU * v);
    });
    let a_end: ?Mutation<nil> = mutate(fn (_ctx: MutationCtx, _e: nil) {
    });
    let ctrl = anim_ctrl(200.0, "linear", 0, a_tick, a_end);
    st_put(K_CTRL, ctrl.raw());
    mount(xf);
    return angle.atom_id();
}

entry fn do_forward(_a: u64, _b: f64) {
    let c = AnimCtrl(st_take(K_CTRL));
    c.forward();
    st_put(K_CTRL, c.raw());
}
"#;

#[test]
fn bound_angle_animates_without_rebuild() {
    let mut app = TurTestApp::new(300.0, 300.0).unwrap();
    app.load_rut_module(BOUND_ANGLE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let xf = ElementNodeId::new(app.query_element(&["rut", "transform"]).unwrap().as_u64());
    let square = ElementNodeId::new(app.query_element(&["bt", "square"]).unwrap().as_u64());
    assert_eq!(painted_rotate(&app, xf), 0.0, "the atom boots at angle 0");
    assert_eq!(transform_count(&app), 1);

    // Play: the controller ticks TAU·v into the atom; halfway through the
    // linear 200ms run the bound angle is ~π — with the element identity
    // UNMOVED (the old rebuild channel re-mounted a fresh Transform per
    // tick, churning the id).
    app.call_rut_entry("do_forward", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::from_millis(100));
    let mid = painted_rotate(&app, xf);
    assert!(
        (mid - std::f64::consts::PI).abs() < 0.2,
        "at t=0.5 the bound angle should be ~π, got {mid}"
    );
    assert_eq!(
        ElementNodeId::new(app.query_element(&["rut", "transform"]).unwrap().as_u64()),
        xf,
        "the transform element identity survives the tick (no rebuild)"
    );
    assert_eq!(
        ElementNodeId::new(app.query_element(&["bt", "square"]).unwrap().as_u64()),
        square,
        "the child identity survives too"
    );
    assert_eq!(transform_count(&app), 1, "no duplicate transform mounted");

    // Completion: the eased value lands at 1 → a full turn, identity intact.
    app.wait_for_timeout(Duration::from_millis(150));
    let end = painted_rotate(&app, xf);
    assert!(
        (end - std::f64::consts::TAU).abs() < 0.05,
        "at completion the bound angle is a full turn, got {end}"
    );
    assert_eq!(
        ElementNodeId::new(app.query_element(&["rut", "transform"]).unwrap().as_u64()),
        xf,
        "identity still stable after completion"
    );
}

/// The static path — `el_transform` with all-static channels — unchanged.
const STATIC_TRANSFORM_RUT: &str = r#"
use tur_host::{ mount };
use tur_kit::{ Container, Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_anim_kit::{ Transform };

entry fn start() {
    let square = Container().width_height(40.0, 40.0).color(0xFFFFFFFFu64).build();
    mount(Transform(1.0, 0.7, 12.0, 0.0).child(square).build());
}
"#;

#[test]
fn static_transform_path_unchanged() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(STATIC_TRANSFORM_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let xf = ElementNodeId::new(app.query_element(&["rut", "transform"]).unwrap().as_u64());
    app.with_element(xf, |el| {
        let t = el.cast::<TransformElement>().unwrap();
        assert_eq!(t.painted_rotate(), 0.7, "the static angle paints verbatim");
        assert_eq!(t.painted_scale(), 1.0, "the static scale paints verbatim");
        let (tx, ty) = t.painted_translate();
        assert_eq!(
            (tx, ty),
            (12.0, 0.0),
            "the static translate paints verbatim"
        );
    });
}

/// The symmetric-cheap twins: `scale_bound` and `translate_bound` ride
/// their atoms the same way (each in its own app — the rut qkey
/// `rut/transform` matches the first transform).
const BOUND_SCALE_RUT: &str = r#"
use tur_host::{ ctx_bridge, mount };
use tur_kit::{ Container, Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_anim_kit::{ Transform };

entry fn start() -> u64 {
    let s: Readable<f64> = source<f64>(2.0);
    let square = Container().width_height(40.0, 40.0).color(0xFFFFFFFFu64).build();
    mount(Transform(1.0, 0.0, 0.0, 0.0).scale_bound(s).child(square).build());
    return s.atom_id();
}

entry fn probe_s(atom: u64, b: f64) {
    let s = Source<f64>.of(ctx_bridge(), atom, false, 1);
    MutationCtx.over(ctx_bridge()).set<f64>(s, b);
}
"#;

const BOUND_TRANSLATE_RUT: &str = r#"
use tur_host::{ ctx_bridge, mount };
use tur_kit::{ Container, Mutation, MutationCtx, Readable, Source, mutate, source };
use tur_anim_kit::{ Transform };

entry fn start() -> u64 {
    let tx: Readable<f64> = source<f64>(10.0);
    let ty: Readable<f64> = source<f64>(20.0);
    let square = Container().width_height(40.0, 40.0).color(0xFFFFFFFFu64).build();
    mount(Transform(1.0, 0.0, 0.0, 0.0).translate_bound(tx, ty).child(square).build());
    return tx.atom_id();
}

// The ty atom mints right after tx — the test drives it at tx+1 (the
// probe takes the pair's head and writes both).
entry fn probe_t(atom: u64, b: f64) {
    // The tx/ty pair mints in order (atom, atom+1) — one entry drives
    // both channels (ty reads 2× the arg, the test's b×2 expectation).
    let write = MutationCtx.over(ctx_bridge());
    let tx = Source<f64>.of(ctx_bridge(), atom, false, 1);
    let ty = Source<f64>.of(ctx_bridge(), atom + 1, false, 1);
    write.set<f64>(tx, b);
    write.set<f64>(ty, b * 2.0);
}
"#;

#[test]
fn scale_and_translate_bounds_follow_their_atoms() {
    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(BOUND_SCALE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let xf = ElementNodeId::new(app.query_element(&["rut", "transform"]).unwrap().as_u64());
    app.with_element(xf, |el| {
        let t = el.cast::<TransformElement>().unwrap();
        assert_eq!(t.painted_scale(), 2.0, "the atom's initial scale");
    });
    let s_atom = app.rut_start_answer();
    app.call_rut_entry("probe_s", s_atom, 3.5).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app.with_element(xf, |el| {
        let t = el.cast::<TransformElement>().unwrap();
        assert_eq!(t.painted_scale(), 3.5, "scale follows the atom");
    });

    let mut app = TurTestApp::new(200.0, 200.0).unwrap();
    app.load_rut_module(BOUND_TRANSLATE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let xf = ElementNodeId::new(app.query_element(&["rut", "transform"]).unwrap().as_u64());
    app.with_element(xf, |el| {
        let t = el.cast::<TransformElement>().unwrap();
        assert_eq!(
            t.painted_translate(),
            (10.0, 20.0),
            "the atoms' initial offsets"
        );
    });
    app.call_rut_entry("probe_t", app.rut_start_answer(), 30.0)
        .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    app.with_element(xf, |el| {
        let t = el.cast::<TransformElement>().unwrap();
        assert_eq!(
            t.painted_translate(),
            (30.0, 60.0),
            "translate follows the atoms"
        );
    });
}

// ---- the corpus complex-animation case ("Animated Card Studio") ---------------

/// The studio card's laid-out width (the width-bound tween target).
fn studio_card_width(app: &TurTestApp) -> f64 {
    let id = app
        .query_element(&["cas-card"])
        .expect("studio card not found");
    let tree = app.element_tree();
    tree.get_element(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .computed_layout
        .size
        .width
}

/// A real click on a keyed studio control (the tap intent path — the
/// transport entries are 3-arg tap targets, only reachable through the
/// pointer rail).
fn tap_studio(app: &mut TurTestApp, key: &str) {
    let id = app
        .query_element(&[key])
        .unwrap_or_else(|| panic!("{key} not found"));
    let b = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .center();
    app.click(b.0, b.1);
    app.wait_for_timeout(Duration::ZERO);
}

#[test]
fn complex_animation_case_runs_the_card_studio() {
    // The showcase case loads standalone (the corpus rail) and drives its
    // whole studio through the real tap rail: boot state, play/pause/resume
    // transport, the % readout, the 4x speed retime, and the loop toggle
    // (the recreate-and-seek path).
    let mut app = TurTestApp::new(500.0, 800.0).unwrap();
    app.load_rut_bundle("complex-animation").unwrap();
    let _progress = app.rut_start_answer();

    // Boot: progress 0 — the card at W_MIN, the badge STOPPED, the readout 0%.
    assert_eq!(
        app.query_text(&["cas-title"]).as_deref(),
        Some("Animated Card Studio")
    );
    assert_eq!(app.query_text(&["cas-status"]).as_deref(), Some("STOPPED"));
    assert_eq!(app.query_text(&["cas-pct"]).as_deref(), Some("0%"));
    assert_eq!(studio_card_width(&app), 120.0, "the card boots at W_MIN");

    // Play: the width + % readout advance with the tick (easeInOut 2400ms;
    // halfway through, the eased value is 0.5 → width 200). The spin rides
    // the bound row — the transform element is NEVER re-mounted while
    // playing (the rebuild channel is gone).
    let xf = app.query_element(&["rut", "transform"]).unwrap();
    tap_studio(&mut app, "cas-play");
    assert_eq!(app.query_text(&["cas-status"]).as_deref(), Some("FORWARD"));
    app.wait_for_timeout(Duration::from_millis(1200));
    assert_eq!(
        app.query_element(&["rut", "transform"]),
        Some(xf),
        "the spinning square stays under ONE transform element while playing (no per-tick rebuild)"
    );
    let w = studio_card_width(&app);
    assert!(
        w > 150.0 && w < 250.0,
        "mid-play the card width should be mid-tween (~200), got {w}"
    );
    assert_ne!(
        app.query_text(&["cas-pct"]).as_deref(),
        Some("0%"),
        "the % readout tracks the tick"
    );

    // Pause freezes at the pause value; resume plays out to COMPLETED.
    tap_studio(&mut app, "cas-pause");
    assert_eq!(app.query_text(&["cas-status"]).as_deref(), Some("PAUSED"));
    let frozen = studio_card_width(&app);
    app.wait_for_timeout(Duration::from_millis(300));
    assert!(
        (studio_card_width(&app) - frozen).abs() < 1.0,
        "paused width stays frozen at {frozen}"
    );
    tap_studio(&mut app, "cas-resume");
    app.wait_for_timeout(Duration::from_millis(1400));
    assert_eq!(
        app.query_text(&["cas-status"]).as_deref(),
        Some("COMPLETED")
    );
    assert_eq!(studio_card_width(&app), 280.0, "completed lands at W_MAX");
    assert_eq!(app.query_text(&["cas-pct"]).as_deref(), Some("100%"));

    // Stop freezes the status; the 4x chip retimes the controller so a
    // fresh forward plays the 2400ms timeline in ~600ms.
    tap_studio(&mut app, "cas-stop");
    assert_eq!(app.query_text(&["cas-status"]).as_deref(), Some("STOPPED"));
    tap_studio(&mut app, "cas-s3");
    tap_studio(&mut app, "cas-play");
    app.wait_for_timeout(Duration::from_millis(700));
    assert_eq!(
        app.query_text(&["cas-status"]).as_deref(),
        Some("COMPLETED"),
        "at 4x the 2400ms timeline completes in ~600ms of wall time"
    );

    // Loop: the recreate-and-seek path swaps in an infinite controller —
    // it keeps cycling (never completes) from any frozen value.
    tap_studio(&mut app, "cas-loop");
    tap_studio(&mut app, "cas-play");
    app.wait_for_timeout(Duration::from_millis(3000));
    assert_eq!(app.query_text(&["cas-status"]).as_deref(), Some("FORWARD"));
    let w = studio_card_width(&app);
    assert!(
        (120.0..=280.0).contains(&w),
        "an infinite loop keeps the width cycling inside the tween range: {w}"
    );
}
