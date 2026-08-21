//! Pins the harness's public single-frame primitive: `TurTestApp::pump`
//! drives the production loop (`TurAppLooper::run`) forward by exactly one frame and
//! returns that frame's `FrameOutcome`.

use std::time::Duration;
use tur_integration_tests::TurTestApp;

#[test]
fn pump_returns_frame_outcome() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("clickable-text").unwrap();
    // A pumped frame on a freshly-loaded tree: the module eval built the
    // tree, so the first pumped frame paints.
    let outcome = app.pump();
    assert!(outcome.painted, "first pumped frame should paint");
    // A quiescent tree with no animation requests no further frames.
    app.wait_for_timeout(Duration::ZERO);
    let idle = app.pump();
    assert!(!idle.painted, "quiesced frame should not paint");
    assert_eq!(
        idle.schedule,
        tur_engine::core::app::NextFrame::Idle,
        "quiesced frame should schedule idle"
    );
}
