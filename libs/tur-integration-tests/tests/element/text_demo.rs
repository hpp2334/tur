//! The `text-demo` corpus case: the Text styling showcase (sizes, weights,
//! colors, the span run, and the interactive maxLines + overflow trio).
//! Pins the case compile + render through the standard plugin set and the
//! maxLines cycle actually swapping the card row.

use std::time::Duration;

use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn build() -> TurTestApp {
    let mut app = TurTestApp::new(600.0, 800.0).unwrap();
    app.load_rut_bundle("text-demo").unwrap();
    app
}

fn click_qk(app: &mut TurTestApp, qk: &[&str]) {
    let id = app
        .query_element(qk)
        .unwrap_or_else(|| panic!("{qk:?} not found"));
    let (cx, cy) = app
        .get_element_absolute_bounds(ElementNodeId::new(id.as_u64()))
        .unwrap()
        .center();
    app.click(cx, cy);
    app.wait_for_timeout(Duration::ZERO);
}

#[test]
fn text_demo_renders_the_styling_showcase() {
    let app = build();
    // The section headers + the caption land.
    assert_eq!(
        app.query_text(&["overflow-caption"]).as_deref(),
        Some("maxLines = 2  ·  width = 100px"),
        "the caption derive seeds at 2",
    );
    // The overflow cards row renders under its query key (the label rides
    // the button, a gesture element — the cycle test drives it by tap).
    assert!(
        app.query_element(&["overflow-cards"]).is_some(),
        "cards row"
    );
}

#[test]
fn text_demo_cycles_max_lines() {
    let mut app = build();
    click_qk(&mut app, &["overflow-btn"]);
    assert_eq!(
        app.query_text(&["overflow-caption"]).as_deref(),
        Some("maxLines = 1  ·  width = 100px"),
        "2 → 1",
    );
    click_qk(&mut app, &["overflow-btn"]);
    assert_eq!(
        app.query_text(&["overflow-caption"]).as_deref(),
        Some("maxLines = 3  ·  width = 100px"),
        "1 → 3",
    );
    click_qk(&mut app, &["overflow-btn"]);
    assert_eq!(
        app.query_text(&["overflow-caption"]).as_deref(),
        Some("maxLines = 2  ·  width = 100px"),
        "3 → 2",
    );
}
