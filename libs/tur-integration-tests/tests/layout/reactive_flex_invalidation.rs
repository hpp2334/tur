use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

/// Reproduces the layout-invalidation gap: when a reactive flex weight changes,
/// only the flex parent (Row) is marked dirty, not its Expanded children.
/// `layout` short-circuits on `dirty_layout` and ignores the new
/// constraints passed by the parent, so the children keep stale cached sizes
/// until some unrelated event marks them dirty.
///
/// Symptom in the playground: switching the Edit/View tab does not relayout
/// the editor/viewer panes until a later click.
#[test]
fn reactive_flex_change_relays_out_children() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("layout-invalidation-reactive-flex")
        .unwrap();

    let a_id = app.query_element(&["a"]).expect("query a");
    let a_id = ElementNodeId::new(a_id.as_u64());
    let b_id = app.query_element(&["b"]).expect("query b");
    let b_id = ElementNodeId::new(b_id.as_u64());

    app.wait_for_timeout(std::time::Duration::ZERO);

    // Initially flex 1:1 in a 400-wide row -> 200 each.
    {
        let tree = app.element_tree();
        assert_eq!(
            tree.get_element(a_id).unwrap().computed_layout.size.width,
            200.0,
            "initial: A should be half"
        );
        assert_eq!(
            tree.get_element(b_id).unwrap().computed_layout.size.width,
            200.0,
            "initial: B should be half"
        );
    }

    // Flip the flex to 3:1 via reactive source sets — no gesture, so no
    // `mark_dirty` is called on any descendant. The pair mints in order
    // (a then b); the probes address them by the start answer + offset.
    let flex_a = app.rut_start_answer();
    app.call_rut_entry("set_flex_a", flex_a, 3.0).unwrap();
    app.call_rut_entry("set_flex_b", flex_a + 1, 1.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    {
        let tree = app.element_tree();
        assert_eq!(
            tree.get_element(a_id).unwrap().computed_layout.size.width,
            300.0,
            "after setFlex(3,1): A should grow to 300"
        );
        assert_eq!(
            tree.get_element(b_id).unwrap().computed_layout.size.width,
            100.0,
            "after setFlex(3,1): B should shrink to 100"
        );
    }
}
