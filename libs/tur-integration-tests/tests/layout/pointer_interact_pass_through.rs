use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn pointer_interact_passes_constraints_and_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var pi = globalThis.__tur.createPointerInteract(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, container, "width", 100);
        globalThis.__tur.setAttribute(ctx, container, "height", 50);
        globalThis.__tur.appendChild(ctx, pi, container);
        globalThis.__tur.appendChild(ctx, root, pi);
    "#,
    )
    .unwrap();

    let (pi_id, container_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        assert_eq!(root.children.len(), 1);

        let pi = tree.get(root.children[0]).unwrap();
        assert_eq!(
            pi.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_pointer_interact")
        );
        assert_eq!(pi.children.len(), 1);

        let container = tree.get(pi.children[0]).unwrap();
        assert_eq!(
            container.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_container")
        );

        (pi.id, container.id)
    };

    let rt = app.render_tree();
    let rt = rt.borrow();

    let pi_node = rt.get(pi_id).unwrap();
    assert_eq!(pi_node.computed_layout.size.width, 100.0);
    assert_eq!(pi_node.computed_layout.size.height, 50.0);

    let container_node = rt.get(container_id).unwrap();
    assert_eq!(container_node.computed_layout.size.width, 100.0);
    assert_eq!(container_node.computed_layout.size.height, 50.0);
    assert_eq!(container_node.computed_layout.offset.x, 0.0);
    assert_eq!(container_node.computed_layout.offset.y, 0.0);
}

#[test]
fn pointer_interact_passes_through_in_column() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
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

        var pi1 = globalThis.__tur.createPointerInteract(ctx);
        var sb1 = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, sb1, "width", 80);
        globalThis.__tur.setAttribute(ctx, sb1, "height", 40);
        globalThis.__tur.appendChild(ctx, pi1, sb1);
        globalThis.__tur.appendChild(ctx, col, pi1);

        var pi2 = globalThis.__tur.createPointerInteract(ctx);
        var sb2 = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, sb2, "width", 60);
        globalThis.__tur.setAttribute(ctx, sb2, "height", 30);
        globalThis.__tur.appendChild(ctx, pi2, sb2);
        globalThis.__tur.appendChild(ctx, col, pi2);
    "#,
    )
    .unwrap();

    let (pi1_id, pi2_id, sb1_id, sb2_id) = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let col = tree.get(root.children[0]).unwrap();
        assert_eq!(col.children.len(), 2);

        let pi1 = tree.get(col.children[0]).unwrap();
        let pi2 = tree.get(col.children[1]).unwrap();
        assert_eq!(
            pi1.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_pointer_interact")
        );
        assert_eq!(
            pi2.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_pointer_interact")
        );

        let sb1 = tree.get(pi1.children[0]).unwrap();
        let sb2 = tree.get(pi2.children[0]).unwrap();

        (pi1.id, pi2.id, sb1.id, sb2.id)
    };

    let rt = app.render_tree();
    let rt = rt.borrow();

    let pi1_node = rt.get(pi1_id).unwrap();
    assert_eq!(pi1_node.computed_layout.size.width, 80.0);
    assert_eq!(pi1_node.computed_layout.size.height, 40.0);
    assert_eq!(pi1_node.computed_layout.offset.y, 0.0);

    let sb1_node = rt.get(sb1_id).unwrap();
    assert_eq!(sb1_node.computed_layout.offset.x, 0.0);
    assert_eq!(sb1_node.computed_layout.offset.y, 0.0);

    let pi2_node = rt.get(pi2_id).unwrap();
    assert_eq!(pi2_node.computed_layout.size.width, 60.0);
    assert_eq!(pi2_node.computed_layout.size.height, 30.0);
    assert_eq!(pi2_node.computed_layout.offset.y, 40.0);

    let sb2_node = rt.get(sb2_id).unwrap();
    assert_eq!(sb2_node.computed_layout.offset.y, 0.0);
}
