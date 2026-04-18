use tur_integration_tests::TurTestApp;

fn print_tree(tree: &tur_element::ElementTree) {
    fn go(tree: &tur_element::ElementTree, id: tur_element::ElementNodeId, depth: usize) {
        if let Some(node) = tree.get(id) {
            eprintln!(
                "{}{:?} id={} children={} props={:?}",
                "  ".repeat(depth),
                node.kind,
                node.id.as_u64(),
                node.children.len(),
                node.props,
            );
            for &child_id in &node.children {
                go(tree, child_id, depth + 1);
            }
        }
    }

    if let Some(root) = tree.root() {
        go(tree, root.id, 0);
    }
}

#[test]
fn debug_raw_column_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var col = globalThis.__tur.create(ctx, "tur_column");
        globalThis.__tur.setAttribute(ctx, col, "crossAlignment", "Start");
        globalThis.__tur.appendChild(ctx, root, col);
        var sb1 = globalThis.__tur.create(ctx, "tur_sized_box");
        globalThis.__tur.setAttribute(ctx, sb1, "height", 50);
        globalThis.__tur.appendChild(ctx, col, sb1);
        var sb2 = globalThis.__tur.create(ctx, "tur_sized_box");
        globalThis.__tur.setAttribute(ctx, sb2, "height", 30);
        globalThis.__tur.appendChild(ctx, col, sb2);
    "#,
    )
    .unwrap();

    eprintln!("=== raw JS: column-basic ===");
    print_tree(&app.element_tree());
}

#[test]
fn debug_solidjs_column_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    eprintln!("=== SolidJS: column-basic ===");
    print_tree(&app.element_tree());
}
