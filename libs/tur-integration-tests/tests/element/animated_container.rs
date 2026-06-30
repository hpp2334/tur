use std::time::Duration;
use tur_engine::core::element::ElementNodeId;

use tur_integration_tests::TurTestApp;

/// Find the first element in the tree whose `type_name` matches.
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

/// First mount: the AnimatedContainer seeds its displayed value to the
/// target with no animation (Flutter first-frame rule).
#[test]
fn animated_container_first_mount_seeds_target() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.width$ = globalThis.__tur.source(ctx, 120);
        var el = globalThis.__tur.AnimatedContainer(ctx, {
            width: globalThis.__tur.width$,
            duration: 200,
        });
        globalThis.__tur.render(ctx, el);
    "#);

    app.render();
    let id = find_first_of_type(&app.element_tree(), "tur_animated_container")
        .expect("AnimatedContainer mounted");
    let w = app
        .element_tree()
        .get_element(id)
        .unwrap()
        .computed_layout
        .size
        .width;
    assert_eq!(
        w, 120.0,
        "first-frame width should equal the target (120), got {w}"
    );
}

/// A target change starts an implicit animation; the displayed value passes
/// through the lerp midpoint at half the duration (linear curve).
#[test]
fn animated_container_animates_to_new_target_linear() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.width$ = globalThis.__tur.source(ctx, 100);
        var el = globalThis.__tur.AnimatedContainer(ctx, {
            width: globalThis.__tur.width$,
            duration: 200,
            curve: "linear",
        });
        globalThis.__tur.render(ctx, el);
    "#);
    app.render();

    // Retarget: 100 -> 200.
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.set(ctx, globalThis.__tur.width$, 200);
    "#);
    app.render();

    let id = find_first_of_type(&app.element_tree(), "tur_animated_container")
        .expect("AnimatedContainer mounted");

    // Advance half the duration → eased_t ≈ 0.5 → width ≈ 150.
    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    let w = app
        .element_tree()
        .get_element(id)
        .unwrap()
        .computed_layout
        .size
        .width;
    assert!(
        (w - 150.0).abs() < 1.0,
        "at half-duration width should be ~150, got {w}"
    );

    // Advance the remaining duration → width = 200.
    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    let w = app
        .element_tree()
        .get_element(id)
        .unwrap()
        .computed_layout
        .size
        .width;
    assert_eq!(w, 200.0, "after duration width should be 200, got {w}");
}

/// `onEnd` fires exactly once when the implicit animation completes.
#[test]
fn animated_container_fires_on_end_once() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.endCount = 0;
        globalThis.__tur.width$ = globalThis.__tur.source(ctx, 100);
        var onEnd = globalThis.__tur.mutate(ctx, function() {
            globalThis.__tur.endCount++;
        });
        var el = globalThis.__tur.AnimatedContainer(ctx, {
            width: globalThis.__tur.width$,
            duration: 100,
            curve: "linear",
            onEnd: onEnd,
        });
        globalThis.__tur.render(ctx, el);
    "#);
    app.render();

    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.set(ctx, globalThis.__tur.width$, 200);
    "#);
    app.render();

    // Before completion: onEnd has not fired.
    let count = app
        .eval_js("String(globalThis.__tur.endCount)")
        .parse::<u32>()
        .unwrap();
    assert_eq!(count, 0, "onEnd must not fire before completion");

    // Advance past duration → animation completes.
    app.advance(Duration::from_millis(150)).unwrap();
    app.render();
    app.render();

    let count = app
        .eval_js("String(globalThis.__tur.endCount)")
        .parse::<u32>()
        .unwrap();
    assert_eq!(count, 1, "onEnd must fire exactly once on completion");

    // Another frame must not re-fire onEnd.
    app.advance(Duration::from_millis(50)).unwrap();
    app.render();
    let count = app
        .eval_js("String(globalThis.__tur.endCount)")
        .parse::<u32>()
        .unwrap();
    assert_eq!(count, 1, "onEnd must not fire again on subsequent frames");
}

/// Color props interpolate channel-wise (mirrors Flutter's ColorTween).
#[test]
fn animated_container_interpolates_color() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.color$ = globalThis.__tur.source(ctx, __tur.createColor(0, 0, 0, 255));
        var el = globalThis.__tur.AnimatedContainer(ctx, {
            width: 50, height: 50,
            color: globalThis.__tur.color$,
            duration: 200,
            curve: "linear",
        });
        globalThis.__tur.render(ctx, el);
    "#);
    app.render();

    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        globalThis.__tur.set(ctx, globalThis.__tur.color$, __tur.createColor(100, 200, 50, 255));
    "#);
    app.render();

    let id = find_first_of_type(&app.element_tree(), "tur_animated_container")
        .expect("AnimatedContainer mounted");

    let read_color = |app: &TurTestApp, id: ElementNodeId| -> (u8, u8, u8) {
        let tree = app.element_tree();
        let node = tree.get_element(id).unwrap();
        let ac = node
            .element
            .as_ref()
            .unwrap()
            .cast::<tur_engine::elements::AnimatedContainerElement>()
            .expect("downcast to AnimatedContainerElement");
        match ac.painting.color.as_ref().unwrap() {
            tur_shared::Brush::SolidColor(c) => (c.r(), c.g(), c.b()),
            _ => panic!("expected solid color"),
        }
    };

    // At t=0 (just retargeted) the displayed color is the old target (black).
    assert_eq!(read_color(&app, id), (0, 0, 0));

    // Advance to midpoint → channels interpolate toward (100, 200, 50).
    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    assert_eq!(
        read_color(&app, id),
        (50, 100, 25),
        "color channels at t=0.5 should be the per-channel midpoint"
    );
}
