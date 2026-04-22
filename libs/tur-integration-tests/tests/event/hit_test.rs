use tur_engine::core::element::ElementNodeId;
use tur_integration_tests::TurTestApp;

fn build_two_containers(app: &mut TurTestApp) -> (ElementNodeId, ElementNodeId, ElementNodeId) {
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        globalThis.__tur.setAttribute(ctx, root, "direction", 0);
        globalThis.__tur.setAttribute(ctx, root, "crossAlignment", 0);
        var col = globalThis.__tur.createFlex(ctx);
        globalThis.__tur.setAttribute(ctx, col, "direction", 0);
        globalThis.__tur.setAttribute(ctx, col, "crossAlignment", 0);
        globalThis.__tur.appendChild(ctx, root, col);

        var c1 = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, c1, "width", 100);
        globalThis.__tur.setAttribute(ctx, c1, "height", 100);
        globalThis.__tur.appendChild(ctx, col, c1);

        var c2 = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, c2, "width", 100);
        globalThis.__tur.setAttribute(ctx, c2, "height", 100);
        globalThis.__tur.appendChild(ctx, col, c2);
    "#,
    )
    .unwrap();

    app.render();
    let tree_rc = app.element_tree();
    let tree = tree_rc.borrow();
    let root = tree.root().unwrap();
    let col_id = root.children[0];
    let col = tree.get(col_id).unwrap();
    let c1_id = col.children[0];
    let c2_id = col.children[1];
    (col_id, c1_id, c2_id)
}

#[test]
fn hit_test_path_first_child() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let (_col_id, c1_id, _c2_id) = build_two_containers(&mut app);

    let tree = app.element_tree();
    let tree = tree.borrow();

    let path = tree.hit_test_path(tur_shared::Offset::new(50.0, 50.0));
    assert!(!path.is_empty());
    assert!(path.contains(&c1_id));
}

#[test]
fn hit_test_path_second_child() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let (_col_id, _c1_id, c2_id) = build_two_containers(&mut app);

    let tree = app.element_tree();
    let tree = tree.borrow();

    let path = tree.hit_test_path(tur_shared::Offset::new(50.0, 150.0));
    assert!(!path.is_empty());
    assert!(path.contains(&c2_id));
}

#[test]
fn hit_test_path_outside_returns_empty() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let _ = build_two_containers(&mut app);

    let tree = app.element_tree();
    let tree = tree.borrow();

    let path = tree.hit_test_path(tur_shared::Offset::new(999.0, 999.0));
    assert!(path.is_empty());
}

#[test]
fn hit_test_path_order_is_leaf_first() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    let (col_id, c1_id, _c2_id) = build_two_containers(&mut app);

    let tree = app.element_tree();
    let tree = tree.borrow();

    let path = tree.hit_test_path(tur_shared::Offset::new(50.0, 50.0));
    assert!(path.len() >= 2);

    let root = tree.root().unwrap();
    assert_eq!(path.first(), Some(&c1_id));
    assert!(path.contains(&col_id));
    assert!(path.contains(&root.id));

    let c1_pos = path.iter().position(|id| *id == c1_id).unwrap();
    let col_pos = path.iter().position(|id| *id == col_id).unwrap();
    let root_pos = path.iter().position(|id| *id == root.id).unwrap();
    assert!(
        c1_pos < col_pos,
        "leaf should come before parent in hit path"
    );
    assert!(
        col_pos < root_pos,
        "child should come before root in hit path"
    );
}
