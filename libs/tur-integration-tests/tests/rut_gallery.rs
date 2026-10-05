//! grid-gallery's chip rail (the phase-7 port): the aspect / density chips
//! re-key ONE Switch over the combined mode string ("1:1/150" …), whose
//! nine lazy cases rebuild the tile grid fresh per mode. Pinned here: the
//! grid STAYS MOUNTED across aspect and density re-keys (a switch branch
//! that mounted ANOTHER switch mid-flush would defer that inner switch's
//! initial case activation — the reason the mode matrix is flat), and the
//! subtitle's selection fragment re-derives on tile taps.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    let (cx, cy) = app.get_element_absolute_bounds(id).unwrap().center();
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
}

#[test]
fn gallery_grid_stays_mounted_across_chip_rekeys() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("grid-gallery").unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Boot: 1:1 × Normal — the grid is mounted, subtitle at tile #0.
    assert!(
        app.query_element(&["gallery", "grid"]).is_some(),
        "the grid mounts at boot"
    );
    assert_eq!(app.query_text(&["gallery", "subtitle"]).as_deref(), Some("tile #0"));
    assert_eq!(app.query_text(&["gallery", "aspect-label"]).as_deref(), Some("1:1"));
    assert_eq!(app.query_text(&["gallery", "extent"]).as_deref(), Some("maxExtent 150"));

    // An aspect re-key rebuilds the grid (the fresh 16:9 case) without a
    // gap — the subtree must never drop.
    click_qk(&mut app, &["gallery", "a-16-9"]);
    assert!(
        app.query_element(&["gallery", "grid"]).is_some(),
        "the grid survives the aspect re-key"
    );
    assert_eq!(
        app.query_text(&["gallery", "aspect-label"]).as_deref(),
        Some("16:9"),
        "the aspect chip re-derives the subtitle"
    );

    // A density re-key on top (16:9 × Dense) — same law, and the density
    // fragment re-derives (the Dense tap really fired).
    click_qk(&mut app, &["gallery", "d-dense"]);
    assert!(
        app.query_element(&["gallery", "grid"]).is_some(),
        "the grid survives the density re-key"
    );
    assert_eq!(
        app.query_text(&["gallery", "extent"]).as_deref(),
        Some("maxExtent 95"),
        "the density chip re-derives the subtitle"
    );

    // And a full round back to 1:1 × Normal.
    click_qk(&mut app, &["gallery", "a-1-1"]);
    click_qk(&mut app, &["gallery", "d-normal"]);
    assert!(
        app.query_element(&["gallery", "grid"]).is_some(),
        "the grid survives the round-trip"
    );
    assert_eq!(app.query_text(&["gallery", "extent"]).as_deref(), Some("maxExtent 150"));
}
