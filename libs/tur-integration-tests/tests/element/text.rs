use tur_engine::core::element::ElementKind;
use tur_integration_tests::TurTestApp;

#[test]
fn text_content_and_measurement() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle("text-basic").unwrap();

    let text_id = {
        let tree_rc = app.element_tree();
        let tree = tree_rc.borrow();
        let root = tree.root().unwrap();
        let text = tree.get(root.children[0]).unwrap();
        assert_eq!(
            text.element.as_ref().unwrap().kind(),
            ElementKind::new("tur_text")
        );
        text.id
    };

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let text_node = rt.get(text_id).unwrap();
    let layout = &text_node.computed_layout;
    assert!(layout.size.width > 0.0, "text width should be positive");
    assert!(layout.size.height > 0.0, "text height should be positive");
}

#[test]
fn text_empty_content_zero_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var text = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, text, "content", "");
        globalThis.__tur.appendChild(ctx, root, text);
    "#,
    )
    .unwrap();

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let root = rt.root().unwrap();
    let text_node = rt.get(root.children[0]).unwrap();
    let layout = &text_node.computed_layout;
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn text_font_size_affects_height() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var t1 = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, t1, "content", "Hello");
        globalThis.__tur.setAttribute(ctx, t1, "fontSize", 14);
        globalThis.__tur.appendChild(ctx, root, t1);
        var t2 = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, t2, "content", "Hello");
        globalThis.__tur.setAttribute(ctx, t2, "fontSize", 28);
        globalThis.__tur.appendChild(ctx, root, t2);
    "#,
    )
    .unwrap();

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let root = rt.root().unwrap();
    let small = rt.get(root.children[0]).unwrap();
    let large = rt.get(root.children[1]).unwrap();
    assert!(
        large.computed_layout.size.height > small.computed_layout.size.height,
        "28px ({}) should be taller than 14px ({})",
        large.computed_layout.size.height,
        small.computed_layout.size.height,
    );
}

#[test]
fn text_wrapping_with_narrow_constraints() {
    let mut app = TurTestApp::new(80.0, 600.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var text = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, text, "content", "Hello World this is a long text that should wrap");
        globalThis.__tur.setAttribute(ctx, text, "fontSize", 14);
        globalThis.__tur.appendChild(ctx, root, text);
    "#,
    )
    .unwrap();

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let root = rt.root().unwrap();
    let text_node = rt.get(root.children[0]).unwrap();
    let layout = &text_node.computed_layout;
    assert!(
        layout.size.height > 30.0,
        "wrapped text should span multiple lines: height={}",
        layout.size.height,
    );
    assert!(
        layout.size.width <= 80.0,
        "width should not exceed 80px constraint: width={}",
        layout.size.width,
    );
}

#[test]
fn text_wrapping_vs_no_wrapping() {
    let js_template = r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var text = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, text, "content", "Hello World this is a long text that should wrap");
        globalThis.__tur.setAttribute(ctx, text, "fontSize", 14);
        globalThis.__tur.appendChild(ctx, root, text);
    "#;

    let mut app_narrow = TurTestApp::new(60.0, 600.0).unwrap();
    app_narrow.load_bundle_raw(js_template).unwrap();
    app_narrow.render();
    let wrapped_height = {
        let rt = app_narrow.element_tree();
        let rt = rt.borrow();
        let root = rt.root().unwrap();
        rt.get(root.children[0])
            .unwrap()
            .computed_layout
            .size
            .height
    };

    let mut app_wide = TurTestApp::new(800.0, 600.0).unwrap();
    app_wide.load_bundle_raw(js_template).unwrap();
    app_wide.render();
    let unwrapped_height = {
        let rt = app_wide.element_tree();
        let rt = rt.borrow();
        let root = rt.root().unwrap();
        rt.get(root.children[0])
            .unwrap()
            .computed_layout
            .size
            .height
    };

    assert!(
        wrapped_height > unwrapped_height,
        "wrapped ({}) should be taller than unwrapped ({})",
        wrapped_height,
        unwrapped_height,
    );
}

#[test]
fn text_in_column_vertical_stacking() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_bundle_raw(
        r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var col = globalThis.__tur.createFlex(ctx);
        globalThis.__tur.setAttribute(ctx, col, "direction", 0);
        globalThis.__tur.setAttribute(ctx, col, "crossAlignment", 2);
        globalThis.__tur.appendChild(ctx, root, col);
        var t1 = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, t1, "content", "First");
        globalThis.__tur.setAttribute(ctx, t1, "fontSize", 14);
        globalThis.__tur.appendChild(ctx, col, t1);
        var t2 = globalThis.__tur.createText(ctx);
        globalThis.__tur.setAttribute(ctx, t2, "content", "Second");
        globalThis.__tur.setAttribute(ctx, t2, "fontSize", 14);
        globalThis.__tur.appendChild(ctx, col, t2);
    "#,
    )
    .unwrap();

    app.render();
    let rt = app.element_tree();
    let rt = rt.borrow();
    let root = rt.root().unwrap();
    let col = rt.get(root.children[0]).unwrap();
    let t1 = rt.get(col.children[0]).unwrap();
    let t2 = rt.get(col.children[1]).unwrap();

    assert_eq!(t1.computed_layout.offset.y, 0.0);
    assert!(
        t2.computed_layout.offset.y >= t1.computed_layout.size.height,
        "second text (y={}) should start below first text (height={})",
        t2.computed_layout.offset.y,
        t1.computed_layout.size.height,
    );
}
