use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn flush(app: &mut TurTestApp) {
    for _ in 0..6 {
        let _ = app.pump();
    }
}

/// Reproduces the pointer-region enter/exit ordering race. When the pointer
/// moves from region A directly to adjacent region B in a single sampled move,
/// the handler diffs the hit-path: exited={A}, entered={B}.
///
/// The mutations are pushed onto a FIFO queue. If `entered` is pushed before
/// `exited`, a shared hover source ends up cleared (set("B") then set("")),
/// because last-write-wins. With exit-before-enter ordering (set("") then
/// set("B")) the final value is correct.
///
/// Symptom in the playground: hovering a case in the sidebar sometimes shows
/// no hover color.
#[test]
fn move_between_adjacent_regions_keeps_target_hovered() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-region-enter-exit-race").unwrap();

    let a_id = app.query_element(&["a"]).expect("query a");
    let a_id = ElementNodeId::new(a_id.as_u64());
    let b_id = app.query_element(&["b"]).expect("query b");
    let b_id = ElementNodeId::new(b_id.as_u64());

    app.render();

    let (ax, ay) = app.get_element_absolute_bounds(a_id).unwrap().center();
    let (bx, by) = app.get_element_absolute_bounds(b_id).unwrap().center();

    // Step 1: move into A to register it in the pointer-region tracker.
    app.pointer_move(ax, ay);
    flush(&mut app);
    assert_eq!(
        app.eval_js("globalThis.__getHover()"),
        "A",
        "after entering A, hover should be 'A'"
    );

    // Step 2: move directly from A to B in a single pointer event. The tracker
    // produces exited={A}, entered={B}. With the wrong push order the shared
    // source is cleared; with exit-before-enter it ends at "B".
    app.pointer_move(bx, by);
    flush(&mut app);
    assert_eq!(
        app.eval_js("globalThis.__getHover()"),
        "B",
        "after moving A->B, hover should be 'B' (not cleared by A's exit)"
    );
}
