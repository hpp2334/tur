use super::vello_app::TurVelloApp;

pub fn vello_counter_app() {
    let app = TurVelloApp::new(256.0, 256.0, 1.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var col = globalThis.__tur.createFlex(ctx);
        globalThis.__tur.appendChild(ctx, root, col);
        var sb1 = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, sb1, "height", 50);
        globalThis.__tur.setAttribute(ctx, sb1, "width", 200);
        globalThis.__tur.appendChild(ctx, col, sb1);
        var sb2 = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, sb2, "height", 30);
        globalThis.__tur.setAttribute(ctx, sb2, "width", 200);
        globalThis.__tur.appendChild(ctx, col, sb2);
    "#,
    )
    .unwrap();

    let tree = app.element_tree();
    let tree = tree.borrow();
    let root = tree.root().unwrap();
    assert!(root.children.len() > 0);
    drop(tree);

    app.render();
    app.present().unwrap();
}
