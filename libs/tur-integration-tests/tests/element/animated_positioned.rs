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

/// First mount: AnimatedPositioned seeds `left` = target (no animation).
#[test]
fn animated_positioned_first_mount_seeds_target() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.left$ = globalThis.__tur.source(ctx, 30);
        var inner = globalThis.__tur.Container(ctx, { width: 40, height: 40 });
        var el = globalThis.__tur.AnimatedPositioned(ctx, {
            left: globalThis.__tur.left$,
            top: 10,
            child: inner,
            duration: 200,
        });
        var stack = globalThis.__tur.Stack(ctx, { children: [el] });
        globalThis.__tur.render(ctx, stack);
    "#);
    app.render();

    let id = find_first_of_type(&app.element_tree(), "tur_animated_positioned")
        .expect("AnimatedPositioned mounted");
    let tree = app.element_tree();
    let node = tree.get_element(id).unwrap();
    // Relative offset within the Stack reflects `left`.
    assert_eq!(node.computed_layout.offset.x, 30.0);
}

/// A target change animates `left` through the midpoint.
#[test]
fn animated_positioned_animates_left() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.left$ = globalThis.__tur.source(ctx, 0);
        var inner = globalThis.__tur.Container(ctx, { width: 40, height: 40 });
        var el = globalThis.__tur.AnimatedPositioned(ctx, {
            left: globalThis.__tur.left$,
            top: 0,
            child: inner,
            duration: 200,
            curve: "linear",
        });
        var stack = globalThis.__tur.Stack(ctx, { children: [el] });
        globalThis.__tur.render(ctx, stack);
    "#);
    app.render();

    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.set(ctx, globalThis.__tur.left$, 200);
    "#);
    app.render();

    let id = find_first_of_type(&app.element_tree(), "tur_animated_positioned")
        .expect("AnimatedPositioned mounted");

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    let x = app
        .element_tree()
        .get_element(id)
        .unwrap()
        .computed_layout
        .offset.x;
    assert!(
        (x - 100.0).abs() < 1.0,
        "at half-duration left should be ~100, got {x}"
    );

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    let x = app
        .element_tree()
        .get_element(id)
        .unwrap()
        .computed_layout
        .offset.x;
    assert_eq!(x, 200.0, "after duration left should be 200, got {x}");
}
