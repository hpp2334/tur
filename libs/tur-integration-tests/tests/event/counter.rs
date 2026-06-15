use tur_engine::elements::Text;
use tur_integration_tests::TurTestApp;

fn get_text(app: &TurTestApp, qk: &[&str]) -> String {
    let id = app.query_element(qk).unwrap();
    app.with_element(id, |e| {
        e.cast::<Text>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn find_pointer_interact(app: &TurTestApp) -> (tur_engine::core::element::ElementNodeId, f64, f64) {
    let all_ids: Vec<tur_engine::core::element::ElementNodeId> = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        fn collect_all(
            tree: &tur_engine::core::elements::ElementTree,
            id: tur_engine::core::element::ElementNodeId,
            out: &mut Vec<tur_engine::core::element::ElementNodeId>,
        ) {
            out.push(id);
            if let Some(node) = tree.get(id) {
                for &child in &node.children {
                    collect_all(tree, child, out);
                }
            }
        }
        let mut ids = Vec::new();
        for &child in &root.children {
            collect_all(&tree, child, &mut ids);
        }
        ids
    };
    let pi_id = all_ids
        .into_iter()
        .find(|&id| app.has_click_handler(id))
        .expect("no clickable element found");
    let (cx, cy) = app.get_element_absolute_bounds(pi_id).unwrap().center();
    (pi_id, cx, cy)
}

#[test]
fn counter_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("counter").unwrap();
    app.render();

    assert_eq!(get_text(&app, &["count"]), "Count: 0");

    let (_pi_id, cx, cy) = find_pointer_interact(&app);

    app.click(cx, cy);
    app.render();

    assert_eq!(get_text(&app, &["count"]), "Count: 1", "should be 1 after click");
}
