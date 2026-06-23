use tur_integration_tests::TurTestApp;

/// Reproduces the position-invalidation gap exposed by dragging the editor /
/// viewer divider in the playground.
///
/// Tree:
///   Container(width = width$)   — reads the reactive source; marked dirty
///     └ Row(mainAlignment=End)  — intermediate descendant, NOT marked dirty,
///                                 but re-measured because its constraints
///                                 changed (its width tracks `width$`).
///       └ Container(width=20)   — "tracker": pushed to the Row's trailing
///                                 edge, so its X offset is `width$ - 20`.
///
/// When `width$` changes, `mark_dirty` walks *up* from the outer container
/// only; the Row re-runs its size phase (constraints changed) but, without
/// also flagging `dirty_position`, the position phase skips it — so the
/// tracker keeps a stale X offset. Symptom in the playground: the editor
/// scrollbar stays painted at its old position (hidden under the viewer pane)
/// after a divider drag.
#[test]
fn reactive_resize_repositions_descendant() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("layout-invalidation-descendant-position")
        .unwrap();

    let tracker_id = app.query_element(&["tracker"]).expect("query tracker");

    app.render();

    // width$ = 100 -> tracker centered: x = (100 - 20) / 2 = 40.
    let initial = app
        .get_element_absolute_bounds(tracker_id)
        .expect("tracker bounds (initial)");
    assert_eq!(
        initial.left, 230.0,
        "initial: width$=100 -> root-centered outer (150) + End-aligned tracker (80)"
    );

    // Change the reactive width WITHOUT a gesture (no extra mark_dirty), then
    // re-render — exactly the divider-drag path.
    app.eval_js("globalThis.__setWidth(300)");
    app.render();

    // width$ = 300:
    //   WITH fix    -> root-centered outer (50) + End-aligned tracker (280) = 330.
    //   WITHOUT fix -> the Row re-measures (width 300) but skips its position
    //                  phase, leaving the tracker at its stale offset (80):
    //                  root-centered outer (50) + 80 = 130.
    let after = app
        .get_element_absolute_bounds(tracker_id)
        .expect("tracker bounds (after)");
    assert_eq!(
        after.left, 330.0,
        "after __setWidth(300): tracker should re-center to x=330 — got x={}. \
         The intermediate Row re-measured but did not re-run its position phase, \
         leaving the descendant at a stale offset.",
        after.left
    );
}
