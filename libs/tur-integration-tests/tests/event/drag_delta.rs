use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// Parse "dsx,dsy,dlx,dly" from the fixture's `__getDragInfo()` into a tuple.
fn drag_info(app: &mut TurTestApp) -> (f64, f64, f64, f64) {
    let s = app.eval_js("globalThis.__getDragInfo()");
    let parts: Vec<f64> = s.split(',').map(|p| p.trim().parse().unwrap_or(9999.0)).collect();
    assert_eq!(parts.len(), 4, "expected 4 comma-separated values, got: {s}");
    (parts[0], parts[1], parts[2], parts[3])
}

/// Verifies that drag-delta tracking produces correct `deltaFromStart` and
/// `deltaFromLast` values across a multi-step drag. This mirrors the
/// `PointerDragEvent` computation used by the playground's `VDivider`.
#[test]
fn drag_delta_from_start_and_from_last_are_correct() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("drag-delta-tracking").unwrap();
    let target = app.query_element(&["drag-target"]).unwrap();
    let target = ElementNodeId::new(target.as_u64());

    app.render();

    let (cx, cy) = app.get_element_absolute_bounds(target).unwrap().center();

    // Press — no drag event yet (onPointerMove hasn't fired).
    app.pointer_down(cx, cy);
    let (dsx, dsy, dlx, dly) = drag_info(&mut app);
    assert_eq!((dsx, dsy, dlx, dly), (0.0, 0.0, 0.0, 0.0), "after down: no delta yet");

    // Move +10x — deltaFromStart = (10,0), deltaFromLast = (10,0).
    app.pointer_move(cx + 10.0, cy);
    let (dsx, dsy, dlx, dly) = drag_info(&mut app);
    assert_eq!((dsx, dsy, dlx, dly), (10.0, 0.0, 10.0, 0.0), "after first move");

    // Move +15x more (total +25x) — deltaFromStart = (25,0), deltaFromLast = (15,0).
    app.pointer_move(cx + 25.0, cy);
    let (dsx, dsy, dlx, dly) = drag_info(&mut app);
    assert_eq!((dsx, dsy, dlx, dly), (25.0, 0.0, 15.0, 0.0), "after second move");

    // Move +5y (total +25x,+5y) — deltaFromStart = (25,5), deltaFromLast = (0,5).
    app.pointer_move(cx + 25.0, cy + 5.0);
    let (dsx, dsy, dlx, dly) = drag_info(&mut app);
    assert_eq!((dsx, dsy, dlx, dly), (25.0, 5.0, 0.0, 5.0), "after vertical move");

    // Release — drag cleared; a subsequent hover-move must not produce deltas.
    app.pointer_up(cx + 25.0, cy + 5.0);
    app.eval_js("globalThis.__resetDrag()");
    app.pointer_move(cx + 40.0, cy + 40.0);
    let (dsx, dsy, dlx, dly) = drag_info(&mut app);
    assert_eq!(
        (dsx, dsy, dlx, dly),
        (0.0, 0.0, 0.0, 0.0),
        "after up: hover-move must not fire onPointerMove / produce deltas"
    );
}
