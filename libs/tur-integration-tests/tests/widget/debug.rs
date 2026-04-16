use tur_integration_tests::TurTestApp;

fn print_tree(tree: &tur_widget::WidgetTree) {
    fn go(tree: &tur_widget::WidgetTree, id: tur_widget::WidgetNodeId, depth: usize) {
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
        var ctx = globalThis.__tur_ctx;
        var root = globalThis.tur_createRoot(ctx);
        var col = globalThis.tur_createElement(ctx, "tur_column");
        globalThis.tur_setAttribute(ctx, col, "crossAlignment", "Start");
        globalThis.tur_appendChild(ctx, root, col);
        var sb1 = globalThis.tur_createElement(ctx, "tur_sized_box");
        globalThis.tur_setAttribute(ctx, sb1, "height", 50);
        globalThis.tur_appendChild(ctx, col, sb1);
        var sb2 = globalThis.tur_createElement(ctx, "tur_sized_box");
        globalThis.tur_setAttribute(ctx, sb2, "height", 30);
        globalThis.tur_appendChild(ctx, col, sb2);
    "#,
    )
    .unwrap();

    eprintln!("=== raw JS: column-basic ===");
    print_tree(&app.widget_tree());
}

#[test]
fn debug_solidjs_column_basic() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("column-basic").unwrap();

    eprintln!("=== SolidJS: column-basic ===");
    print_tree(&app.widget_tree());
}
