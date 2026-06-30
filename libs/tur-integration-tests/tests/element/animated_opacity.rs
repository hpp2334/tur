use std::time::Duration;
use tur_engine::core::element::ElementNodeId;

use tur_integration_tests::TurTestApp;

fn find_first_of_type(
    tree: &tur_engine::core::elements::NodeTreeData,
    type_name: &str,
) -> Option<ElementNodeId> {
    for id in tree.element_ids() {
        if let Some(node) = tree.get_element(id) {
            if node.element.as_ref().map(|e| e.type_name()) == Some(type_name) {
                return Some(id);
            }
        }
    }
    None
}

/// First mount: AnimatedOpacity seeds value = target (no animation from 1.0).
#[test]
fn animated_opacity_first_mount_seeds_target() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.opacity$ = globalThis.__tur.source(ctx, 0.3);
        var inner = globalThis.__tur.Container(ctx, { width: 50, height: 50 });
        var el = globalThis.__tur.AnimatedOpacity(ctx, {
            value: globalThis.__tur.opacity$,
            duration: 200,
            child: inner,
        });
        globalThis.__tur.render(ctx, el);
    "#);
    app.render();

    let id = find_first_of_type(&app.element_tree(), "tur_animated_opacity")
        .expect("AnimatedOpacity mounted");
    let tree = app.element_tree();
    let node = tree.get_element(id).unwrap();
    let el = node
        .element
        .as_ref()
        .unwrap()
        .cast::<tur_engine::elements::AnimatedOpacityElement>()
        .unwrap();
    assert!(
        (el.painting - 0.3).abs() < 1e-6,
        "first-frame opacity should be 0.3, got {}",
        el.painting
    );
}

/// A target change animates opacity through the midpoint.
#[test]
fn animated_opacity_animates_to_new_target() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.opacity$ = globalThis.__tur.source(ctx, 0.0);
        var inner = globalThis.__tur.Container(ctx, { width: 50, height: 50 });
        var el = globalThis.__tur.AnimatedOpacity(ctx, {
            value: globalThis.__tur.opacity$,
            duration: 200,
            curve: "linear",
            child: inner,
        });
        globalThis.__tur.render(ctx, el);
    "#);
    app.render();

    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.set(ctx, globalThis.__tur.opacity$, 1.0);
    "#);
    app.render();

    let id = find_first_of_type(&app.element_tree(), "tur_animated_opacity")
        .expect("AnimatedOpacity mounted");

    let read_opacity = |app: &TurTestApp, id: ElementNodeId| -> f32 {
        let tree = app.element_tree();
        let node = tree.get_element(id).unwrap();
        node.element
            .as_ref()
            .unwrap()
            .cast::<tur_engine::elements::AnimatedOpacityElement>()
            .unwrap()
            .painting
    };

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    let o = read_opacity(&app, id);
    assert!(
        (o - 0.5).abs() < 0.02,
        "at half-duration opacity should be ~0.5, got {o}"
    );

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    let o = read_opacity(&app, id);
    assert!(
        (o - 1.0).abs() < 1e-3,
        "after duration opacity should be 1.0, got {o}"
    );
}
