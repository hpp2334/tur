//! Regression: after a touch drag ends, a second touch drag started shortly
//! after must still register its `onPointerDown` / `onPointerMove`.
//!
//! Reproduces the jigsaw-puzzle symptom ("after dragging and releasing a
//! tile, no tile can be dragged again for ~1-2s"). The `drag-with-lift`
//! fixture mirrors the puzzle's mechanic — `PointerInteract` with a
//! `createAnimationController` lift (forward on down, reverse on up) — and we
//! drive it with **touch** events (which go through the gesture arena, unlike
//! the mouse path the existing drag_delta test exercises).

use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn last_event(app: &TurTestApp) -> String {
    app.eval_js("globalThis.__getLastEvent()")
}

/// Drive a touch down → moves → up sequence with explicit, increasing
/// timestamps (the arena uses these for slop + fling velocity tracking).
fn touch_drag(app: &mut TurTestApp, start: (f64, f64), end: (f64, f64), steps: usize) {
    app.bump_synthetic_time_ms_for_test(40);
    let mut t = app.last_synthetic_time_ms();
    app.push_touch_down(start.0, start.1, t);
    app.wait_for_timeout(std::time::Duration::ZERO);
    for i in 1..=steps {
        app.bump_synthetic_time_ms_for_test(16);
        t = app.last_synthetic_time_ms();
        let frac = i as f64 / steps as f64;
        let x = start.0 + (end.0 - start.0) * frac;
        let y = start.1 + (end.1 - start.1) * frac;
        app.push_touch_move(x, y, t);
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }
    app.bump_synthetic_time_ms_for_test(16);
    t = app.last_synthetic_time_ms();
    app.push_touch_up(end.0, end.1, t);
    app.wait_for_timeout(std::time::Duration::from_millis(16));
}

/// Baseline: two consecutive touch drags on a plain PointerInteract (no lift
/// animation). Confirms the arena itself allows back-to-back drags.
#[test]
fn second_touch_drag_after_release_still_registers() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("drag-delta-tracking").unwrap();
    let target = app.query_element(&["drag-target"]).unwrap();
    let target = ElementNodeId::new(target.as_u64());
    app.wait_for_timeout(std::time::Duration::ZERO);
    let (cx, cy) = app.get_element_absolute_bounds(target).unwrap().center();

    touch_drag(&mut app, (cx, cy), (cx + 40.0, cy + 40.0), 4);
    touch_drag(&mut app, (cx, cy), (cx + 30.0, cy + 30.0), 4);

    // After the second drag the deltas should be non-zero (the second drag's
    // moves registered). drag-delta-tracking exposes deltas via __getDragInfo.
    let s = app.eval_js("globalThis.__getDragInfo()");
    let parts: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse().unwrap_or(9999.0))
        .collect();
    let (dsx, dsy, _, _) = (parts[0], parts[1], parts[2], parts[3]);
    assert!(
        dsx.abs() > 0.0 || dsy.abs() > 0.0,
        "second drag right after release should still register; got ({dsx},{dsy})"
    );
}

/// The jigsaw reproduction: drag a piece whose pointer-down drives a lift
/// animation (forward on down, reverse on up). The reverse animation from the
/// first release is still settling when the second drag starts — if that
/// interferes with the second drag's `onPointerDown`, the second drag won't
/// fire its move events.
#[test]
fn drag_with_lift_second_drag_after_release_registers() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("drag-with-lift").unwrap();
    let target = app.query_element(&["lift-target"]).unwrap();
    let target = ElementNodeId::new(target.as_u64());
    app.wait_for_timeout(std::time::Duration::ZERO);
    let (cx, cy) = app.get_element_absolute_bounds(target).unwrap().center();

    // First drag — should fire down + move + up.
    touch_drag(&mut app, (cx, cy), (cx + 40.0, cy + 40.0), 4);
    assert_eq!(
        last_event(&app),
        "up",
        "first drag should have ended with up"
    );

    // Immediately start a second drag (the lift's reverse animation is still
    // settling — LIFT_MS = 180ms, well within the second drag's window).
    app.eval_js("globalThis.__resetDrag()");
    touch_drag(&mut app, (cx, cy), (cx + 30.0, cy + 30.0), 4);

    assert_eq!(
        last_event(&app),
        "up",
        "second drag right after release should still fire (jigsaw bug); last event was {}",
        last_event(&app)
    );
}

/// Multi-tile repro of the jigsaw symptom. Two tiles share a SINGLE lift
/// controller (`dragScale$` + `liftCtrl`, exactly like the puzzle). Drag tile 0,
/// release, then immediately drag tile 1. The second drag must still fire
/// `onPointerDown` + `onPointerMove` on tile 1 even though tile 0's lift
/// `reverse()` is still settling and grabs the shared controller mid-flight.
///
/// On-device the symptom is "after dragging + releasing a tile, no tile can be
/// dragged again for ~1-2s". This test pins the engine-level behavior: if it
/// regresses, the second drag's events on tile 1 stay empty.
#[test]
fn multi_tile_second_drag_on_other_tile_registers() {
    let mut app = TurTestApp::new(300.0, 200.0).unwrap();
    app.load_bundle("drag-redrag-multi").unwrap();

    let id0 = app.query_element(&["tile-0"]).unwrap();
    let id1 = app.query_element(&["tile-1"]).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let (cx0, cy0) = app
        .get_element_absolute_bounds(ElementNodeId::new(id0.as_u64()))
        .unwrap()
        .center();
    let (cx1, cy1) = app
        .get_element_absolute_bounds(ElementNodeId::new(id1.as_u64()))
        .unwrap()
        .center();

    // First drag: tile 0 (exceeds slop → drag wins → down/move/up).
    touch_drag(&mut app, (cx0, cy0), (cx0 + 40.0, cy0 + 40.0), 4);
    let ev0 = app.eval_js("globalThis.__getTileEvents(0)");
    assert!(
        ev0.contains("down") && ev0.contains("move"),
        "first drag on tile 0 should fire down+move; events were {ev0}"
    );

    // Immediately start a second drag on tile 1. Tile 0's reverse lift is
    // still settling (LIFT_MS = 180ms), so the shared `liftCtrl.forward()` in
    // tile 1's onPointerDown fires mid-reverse.
    app.eval_js("globalThis.__resetDrag()");
    touch_drag(&mut app, (cx1, cy1), (cx1 + 40.0, cy1 + 40.0), 4);

    let ev1 = app.eval_js("globalThis.__getTileEvents(1)");
    assert!(
        ev1.contains("down") && ev1.contains("move"),
        "second drag on tile 1 should fire down+move (jigsaw multi-tile bug); events were {ev1}"
    );
}
