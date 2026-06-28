use tur_engine::core::element::ElementNodeId;
use tur_engine::elements::TextElement;
use tur_integration_tests::TurTestApp;

fn build_nested() -> TurTestApp {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("pointer-interact-nested").unwrap();
    app
}

fn get_text_content(app: &TurTestApp, query_key: &[&str]) -> String {
    let id = app.query_element(query_key).unwrap_or_else(|| panic!("{:?} not found", query_key));
    app.with_element(id, |e| {
        e.cast::<TextElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn find_inner_opaque(app: &TurTestApp) -> (tur_engine::core::element::NodeId, tur_engine::core::element::NodeId) {
    let outer_id = app.query_element(&["outer-opaque"]).unwrap();
    let inner_id = app.query_element(&["inner-opaque"]).unwrap();
    let tree = app.element_tree();
    let inner_container = tree.get_element(ElementNodeId::new(inner_id.as_u64())).unwrap();
    let pi_inner = tree.get_element(ElementNodeId::new(inner_container.parent.unwrap().as_u64())).unwrap();
    let outer_container = tree.get_element(ElementNodeId::new(outer_id.as_u64())).unwrap();
    let pi_outer = tree.get_element(ElementNodeId::new(outer_container.parent.unwrap().as_u64())).unwrap();
    (pi_outer.id.into(), pi_inner.id.into())
}

fn find_inner_translucent(app: &TurTestApp) -> (tur_engine::core::element::NodeId, tur_engine::core::element::NodeId) {
    let outer_id = app.query_element(&["outer-translucent"]).unwrap();
    let inner_id = app.query_element(&["inner-translucent"]).unwrap();
    let tree = app.element_tree();
    let inner_container = tree.get_element(ElementNodeId::new(inner_id.as_u64())).unwrap();
    let pi_inner = tree.get_element(ElementNodeId::new(inner_container.parent.unwrap().as_u64())).unwrap();
    let outer_container = tree.get_element(ElementNodeId::new(outer_id.as_u64())).unwrap();
    let pi_outer = tree.get_element(ElementNodeId::new(outer_container.parent.unwrap().as_u64())).unwrap();
    (pi_outer.id.into(), pi_inner.id.into())
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
