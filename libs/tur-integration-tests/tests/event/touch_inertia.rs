use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_engine::builtin_plugins::scroll::ScrollViewElement;
use tur_integration_tests::TurTestApp;

/// `scroll-view-basic`: 400x300 viewport, Column of 3× 200px SizedBoxes =
/// 600px content → max scroll extent 300.
fn setup_basic() -> (TurTestApp, ElementNodeId, NodeId) {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_bundle("scroll-view-basic").unwrap();

    let (sv_id, col_id) = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        let sv = tree
            .get_element(ElementNodeId::new(root.children[0].as_u64()))
            .unwrap();
        (sv.id, sv.children[0])
    };

    app.render();
    (app, sv_id, col_id)
}

fn scroll_offset(app: &TurTestApp, sv_id: ElementNodeId) -> f64 {
    app.with_element(sv_id, |e| {
        e.cast::<ScrollViewElement>().unwrap().scroll_offset()
    })
    .unwrap()
}

/// Baseline: a touch drag scrolls the content. If this fails, the drag→scroll
/// pipeline is broken and the inertia tests below are meaningless.
#[test]
fn touch_drag_scrolls_content() {
    let (mut app, sv_id, _) = setup_basic();

    // Drag finger UP (y 250 -> 150): content scrolls DOWN, offset increases.
    app.touch_drag((200.0, 250.0), (200.0, 150.0), 8);

    let off = scroll_offset(&app, sv_id);
    assert!(
        off > 50.0,
        "touch drag should have scrolled content, offset={off}"
    );
}

/// The core inertia assertion: after the finger lifts, the offset must keep
/// increasing (kinetic coasting). `after_coast` must exceed `after_release`
/// by a visible margin.
#[test]
fn touch_fling_coasts_after_release() {
    let (mut app, sv_id, _) = setup_basic();

    app.touch_drag((200.0, 250.0), (200.0, 130.0), 10);
    let after_release = scroll_offset(&app, sv_id);

    // Let the inertia simulation coast (~0.5 s of virtual time).
    app.wait_frames(30);
    let after_coast = scroll_offset(&app, sv_id);

    assert!(
        after_coast > after_release + 20.0,
        "inertia should coast beyond the drag delta: after_release={after_release}, after_coast={after_coast}"
    );
}

/// A fresh touch during the coast must cancel the inertia (offset freezes).
#[test]
fn touch_fling_cancels_on_touch() {
    let (mut app, sv_id, _) = setup_basic();

    app.touch_drag((200.0, 250.0), (200.0, 130.0), 10);
    // Let the coast get underway.
    app.wait_frames(6);
    let mid_coast = scroll_offset(&app, sv_id);

    // Finger down to grab — should kill the inertia.
    app.touch_down(200.0, 200.0);
    app.wait_frames(20);
    let after_cancel = scroll_offset(&app, sv_id);

    assert!(
        (after_cancel - mid_coast).abs() < 1.0,
        "fresh touch should freeze the coast: mid_coast={mid_coast}, after_cancel={after_cancel}"
    );
}

/// Regression test for the real-world intermittent failure: a mobile browser
/// coalesces several `touchmove`s and the engine drains them all in a single
/// frame. Before the fix, the gesture subsystem sampled `clock.now()` for each
/// drained event — identical within one flush — so the velocity window filled
/// with same-timestamp samples and the fling velocity came out zero ("cannot
/// trigger every time"). Each event now carries its own `time_ms`
/// (`event.timeStamp`), so even a fully-batched drag yields a real velocity.
///
/// Here the whole down→moves→up sequence is queued with distinct event
/// timestamps but the deterministic clock is **never advanced** during the
/// drag, then drained in a single `pump()`.
#[test]
fn touch_fling_with_batched_moves() {
    let (mut app, sv_id, _) = setup_basic();

    // Queue the entire drag with distinct real event timestamps, no clock
    // advance (simulating one frame draining a coalesced batch).
    app.push_touch_down(200.0, 250.0, 0);
    app.push_touch_move(200.0, 235.0, 16);
    app.push_touch_move(200.0, 210.0, 32);
    app.push_touch_move(200.0, 180.0, 48);
    app.push_touch_move(200.0, 155.0, 64);
    app.push_touch_move(200.0, 135.0, 80);
    app.push_touch_up(200.0, 130.0, 96);

    // Drain everything in a single frame — the FixedClock stays at 0, so a
    // drain-time-sampling implementation would see one timestamp for all.
    let _ = app.pump();
    let after_release = scroll_offset(&app, sv_id);

    // Coast on virtual time.
    app.wait_frames(30);
    let after_coast = scroll_offset(&app, sv_id);

    assert!(
        after_coast > after_release + 20.0,
        "batched drag should still fling: after_release={after_release}, after_coast={after_coast}"
    );
}
