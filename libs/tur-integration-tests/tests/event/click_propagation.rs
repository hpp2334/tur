use tur_engine::elements::TextSpanElement;
use tur_integration_tests::TurTestApp;

fn build_nested() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-interact-nested").unwrap();
    app
}

fn get_text_content(app: &TurTestApp, query_key: &[&str]) -> String {
    let id = app.query_element(query_key).unwrap_or_else(|| panic!("{:?} not found", query_key));
    let tree = app.element_tree();
    let container = tree.get(id).unwrap();
    let span_id = container.children.first().copied();
    drop(tree);
    span_id
        .and_then(|sid| {
            app.with_element(sid, |e| {
                e.cast::<TextSpanElement>()
                    .map(|s| s.content().to_string())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
}

fn find_inner_opaque(app: &TurTestApp) -> (tur_engine::core::element::ElementNodeId, tur_engine::core::element::ElementNodeId) {
    let outer_id = app.query_element(&["outer-opaque"]).unwrap();
    let inner_id = app.query_element(&["inner-opaque"]).unwrap();
    let tree = app.element_tree();
    let inner_container = tree.get(inner_id).unwrap();
    let pi_inner = tree.get(inner_container.parent.unwrap()).unwrap();
    let outer_container = tree.get(outer_id).unwrap();
    let pi_outer = tree.get(outer_container.parent.unwrap()).unwrap();
    (pi_outer.id, pi_inner.id)
}

fn find_inner_translucent(app: &TurTestApp) -> (tur_engine::core::element::ElementNodeId, tur_engine::core::element::ElementNodeId) {
    let outer_id = app.query_element(&["outer-translucent"]).unwrap();
    let inner_id = app.query_element(&["inner-translucent"]).unwrap();
    let tree = app.element_tree();
    let inner_container = tree.get(inner_id).unwrap();
    let pi_inner = tree.get(inner_container.parent.unwrap()).unwrap();
    let outer_container = tree.get(outer_id).unwrap();
    let pi_outer = tree.get(outer_container.parent.unwrap()).unwrap();
    (pi_outer.id, pi_inner.id)
}

#[test]
fn opaque_inner_blocks_outer_click() {
    let mut app = build_nested();
    app.render();

    assert_eq!(get_text_content(&app, &["result-opaque"]), "opaque:0/0");

    let (_, inner_id) = find_inner_opaque(&app);
    let (cx, cy) = app.get_element_absolute_bounds(inner_id).unwrap().center();
    app.click(cx, cy);

    assert_eq!(get_text_content(&app, &["result-opaque"]), "opaque:0/1");
}

#[test]
fn translucent_inner_allows_outer_click() {
    let mut app = build_nested();
    app.render();

    assert_eq!(get_text_content(&app, &["result-translucent"]), "translucent:0/0");

    let (_, inner_id) = find_inner_translucent(&app);
    let (cx, cy) = app.get_element_absolute_bounds(inner_id).unwrap().center();
    app.click(cx, cy);

    assert_eq!(get_text_content(&app, &["result-translucent"]), "translucent:1/1");
}
