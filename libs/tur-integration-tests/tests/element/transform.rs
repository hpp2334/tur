use tur_engine::core::element::ElementKind;
use tur_engine::core::elements::NodeTreeSnapshot;
use tur_integration_tests::TurTestApp;

/// Count nodes of a given element kind anywhere in the tree.
fn count_kind(tree: &NodeTreeSnapshot, kind: &str) -> usize {
    let want = ElementKind::new(kind);
    tree.element_ids()
        .iter()
        .filter(|id| {
            tree.get_element(**id)
                .map(|n| n.kind() == Some(want.clone()))
                .unwrap_or(false)
        })
        .count()
}

/// Regression for the latent `Transform` hit-test gap: a box laid out at
/// (0,0) but painted at (100,80) via `Transform` `translateX`/`translateY`
/// (a paint-only translate) must be clickable at its **painted** center
/// (120,100). Before the `relative_transform` hit-test fix, hit-testing used
/// only the layout offset and ignored the transform — so the click missed.
/// The `Condition`-mounted red box appearing proves the `onClick` fired.
#[test]
fn transform_translate_is_hittable_at_painted_position() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("transform-translate-hit").unwrap();

    // Before the click: only the indigo box (no red mounted box yet).
    let red_before = {
        let tree = app.element_tree();
        count_kind(&tree, "tur_container")
    };

    // Click the PAINTED center (120,100). Layout placed the box at (0,0); the
    // transform translateX/Y=100/80 moved it to (100,80). A transform-unaware
    // hit-test would look at (0,0) and miss.
    app.click(120.0, 100.0);

    let red_after = {
        let tree = app.element_tree();
        count_kind(&tree, "tur_container")
    };
    assert_eq!(
        red_after,
        red_before + 1,
        "click at the transform-translated position (120,100) should have fired onClick and mounted the red box — got {red_before} → {red_after}"
    );
}
