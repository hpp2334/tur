use tur_engine::elements::TextContainerElement;
use tur_integration_tests::TurTestApp;

fn get_text(app: &TurTestApp, qk: &[&str]) -> String {
    let id = app.query_element(qk).unwrap();
    app.with_element(id, |e| {
        e.cast::<TextContainerElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn find_counter_button(app: &TurTestApp) -> (tur_engine::core::element::ElementNodeId, f64, f64) {
    let tree = app.element_tree();
    let root = tree.root().unwrap();
    let container = tree.get(root.children[0]).unwrap();
    let col = tree.get(container.children[0]).unwrap();
    let pi_id = col.children[0];
    drop(tree);
    let (cx, cy) = app.get_element_absolute_bounds(pi_id).unwrap().center();
    (pi_id, cx, cy)
}

#[test]
fn counter_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("counter").unwrap();
    app.render();

    assert_eq!(get_text(&app, &["count"]), "Count: 0");

    let (_pi_id, cx, cy) = find_counter_button(&app);

    app.click(cx, cy);
    app.render();

    assert_eq!(get_text(&app, &["count"]), "Count: 1", "should be 1 after click");
}
