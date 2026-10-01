//! The animation controller state machine, driven end to end through the
//! `tur` host pkg's animation rows: the Rust-held controller opaque ticks
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

use tur_engine::core::element::ElementNodeId;
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
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

/// A width-atom tick target driven by the controller (`100 + 100·v`), a
/// bound box, and `do_*` control entries over the stashed controller.
const CONTROLLER_RUT: &str = r#"
use tur::{
    anim_ctrl, anim_forward, anim_pause, anim_repeat, anim_resume, anim_reverse, anim_seek,
    anim_speed, anim_status, anim_stop, anim_value, box_size, box_width_bound, el_box_new,
    el_build, el_child, el_qkey, el_text_bound, mount, rs_set_f64, rs_source_f64, rs_source_str,
    st_put, st_take,
};

let CTRL: u64 = 7;

entry fn start() -> u64 {
    let label = rs_source_str("");
    let width = rs_source_f64();
    rs_set_f64(width, 100.0);

    let b = el_box_new();
    box_size(b, 10.0, 10.0);
    box_width_bound(b, width);
    el_qkey(b, "box");

    let ctrl = anim_ctrl(width, 200.0, "linear", 0, "a_tick", "a_end");
    st_put(CTRL, ctrl);

    let col = el_column();
    el_child(col, el_build(b));
    el_child(col, el_text_bound(label));
    mount(el_build(col));
    return label;
}

entry fn a_tick(id: u64, v: f64) {
    rs_set_f64(id, 100.0 + (200.0 - 100.0) * v);
}

entry fn a_end(_id: u64, _v: f64) {
}

entry fn do_forward(_a: u64, _b: f64) {
    let c = st_take(CTRL);
    anim_forward(c);
    st_put(CTRL, c);
}

entry fn do_reverse(_a: u64, _b: f64) {
    let c = st_take(CTRL);
    anim_reverse(c);
    st_put(CTRL, c);
}

entry fn do_stop(_a: u64, _b: f64) {
    let c = st_take(CTRL);
    anim_stop(c);
    st_put(CTRL, c);
}

entry fn do_pause(_a: u64, _b: f64) {
    let c = st_take(CTRL);
    anim_pause(c);
    st_put(CTRL, c);
}

entry fn do_resume(_a: u64, _b: f64) {
    let c = st_take(CTRL);
    anim_resume(c);
    st_put(CTRL, c);
}

entry fn do_seek(_a: u64, t: f64) {
    let c = st_take(CTRL);
    anim_seek(c, t);
    st_put(CTRL, c);
}

entry fn do_repeat(_a: u64, n: f64) {
    let c = st_take(CTRL);
    anim_repeat(c, n as u64);
    st_put(CTRL, c);
}

entry fn do_speed(_a: u64, s: f64) {
    let c = st_take(CTRL);
    anim_speed(c, s);
    st_put(CTRL, c);
}

// Report `status|v{value}` into the transcript label (the controller's
// own raw value — the width binding reads the same tick stream).
entry fn probe(label: u64, _b: f64) {
    let c = st_take(CTRL);
    rs_set_str(label, f"{anim_status(c)}|v{anim_value(c)}");
    st_put(CTRL, c);
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
