//! Regression: a non-selectable `Text` painted on top of a `PointerInteract`
//! must NOT steal touch drags from it.
//!
//! The jigsaw-puzzle pieces are `Positioned → Transform → PointerInteract →
//! Container → Text`. On **touch** drags the gesture arena probes the hit-path
//! top-down and the first gesture-capable element that claims (returns `true`)
//! wins. `TextElement` used to be built with a gesture handler that returned
//! `true` even when non-selectable — so the piece's number `Text` won the
//! probe and the `PointerInteract` beneath never received `onPointerDown`
//! (the drag appeared dead). Mouse drags were unaffected because the mouse
//! path dispatches to the whole hit-path without a claim probe.

use tur_integration_tests::TurTestApp;

fn center_of(app: &TurTestApp, query: &[&str]) -> (f64, f64) {
    let id = app.query_element(query).unwrap();
    app.get_element_absolute_bounds(id.as_element_id())
        .unwrap()
        .center()
}

fn down_count(app: &TurTestApp) -> u32 {
    app.eval_js("globalThis.__getDownCount()")
        .parse::<u32>()
        .unwrap_or(0)
}

/// Mouse drag in `steps` moves. Uses the mouse pointer helpers (which bypass
/// the arena's claim probe — the control path).
fn mouse_drag(app: &mut TurTestApp, start: (f64, f64), end: (f64, f64), steps: usize) {
    app.pointer_down(start.0, start.1);
    app.wait_for_timeout(std::time::Duration::ZERO);
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        app.pointer_move(
            start.0 + (end.0 - start.0) * t,
            start.1 + (end.1 - start.1) * t,
        );
        app.wait_for_timeout(std::time::Duration::ZERO);
    }
    app.pointer_up(end.0, end.1);
    app.wait_for_timeout(std::time::Duration::ZERO);
}

/// Mouse drag fires onPointerDown (control — the mouse path bypasses the
/// claim probe). If this fails the harness itself is broken.
#[test]
fn mouse_drag_over_text_fires_pointer_down() {
    let mut app = TurTestApp::new(400.0, 400.0).unwrap();
    app.load_bundle("drag-over-text").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let (cx, cy) = center_of(&app, &["drag-target"]);

    mouse_drag(&mut app, (cx, cy), (cx + 40.0, cy + 40.0), 4);

    assert!(
        down_count(&app) >= 1,
        "mouse drag should fire onPointerDown; got {}",
        down_count(&app)
    );
}

/// The bug: a touch drag over a non-selectable Text must still fire
/// onPointerDown on the PointerInteract beneath. Before the fix the Text
/// claimed the gesture and `down_count` stayed 0.
#[test]
fn touch_drag_over_text_fires_pointer_down() {
    let mut app = TurTestApp::new(400.0, 400.0).unwrap();
    app.load_bundle("drag-over-text").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let (cx, cy) = center_of(&app, &["drag-target"]);

    app.touch_drag((cx, cy), (cx + 40.0, cy + 40.0), 4);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        down_count(&app) >= 1,
        "touch drag over non-selectable Text should fire onPointerDown on the PointerInteract beneath (Text was stealing the drag); down_count={}",
        down_count(&app)
    );
}
