use tur_engine::builtin_plugins::text::elements::TextElement;
use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn get_text(app: &TurTestApp, qk: &[&str]) -> String {
    let id = app.query_element(qk).unwrap();
    let id = ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<TextElement>()
            .map(|c| {
                c.spans()
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn find_pointer_interact(app: &TurTestApp) -> (ElementNodeId, f64, f64) {
    let pi_id = app.query_element(&["inc"]).expect("inc button not found");
    let pi_id = ElementNodeId::new(pi_id.as_u64());
    let (cx, cy) = app.get_element_absolute_bounds(pi_id).unwrap().center();
    (pi_id, cx, cy)
}

#[test]
fn counter_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("counter").unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(get_text(&app, &["count"]), "Count: 0");

    let (_pi_id, cx, cy) = find_pointer_interact(&app);

    app.click(cx, cy);
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        get_text(&app, &["count"]),
        "Count: 1",
        "should be 1 after click"
    );
}
