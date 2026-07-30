use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::elements::NodeTreeData;
use tur_integration_tests::TurTestApp;

/// Walk from `id` depth-first; return the first `tur_paragraph` node's width.
fn find_text_width(tree: &NodeTreeData, id: ElementNodeId) -> Option<f64> {
    let node = tree.get_element(id)?;
    if node.element.as_ref().map(|e| e.kind()) == Some(ElementKind::new("tur_paragraph")) {
        return Some(node.computed_layout.size.width);
    }
    for c in &node.children {
        if let Some(w) = find_text_width(tree, ElementNodeId::new(c.as_u64())) {
            return Some(w);
        }
    }
    None
}

/// Regression guard: a reactive `Text` child nested inside a `ReadableSubscribe`
/// wrapper must still receive reactive updates. `ReadableSubscribe` is a
/// transparent pass-through; it must NOT isolate its subtree from the reactive
/// graph. Builds `ReadableSubscribe > Container > Text(derive(flag))`, flips
/// `flag`, and asserts the Text width changes (proving the derive recomputed).
#[test]
fn readable_subscribe_propagates_reactive_updates_to_child() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, derive, Container, Text, ReadableSubscribe, mutate, render } from "tur:std";
        globalThis.__flag = source(false);
        const flag = globalThis.__flag;
        const cardText = derive(function (g) {
            return g.get(flag) ? "EXPANDED_LABEL_LONG" : "short";
        });
        const inner = Container({
            children: [ Text({ text: cardText, fontSize: 16 }) ]
        });
        const tree = ReadableSubscribe({
            readables: [flag],
            onUpdate$: mutate(function () {}),
            child: inner
        });
        render(tree);
    "#)
    .unwrap();

    app.render();
    let w1 = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_text_width(&tree, root.id).expect("Text should be mounted")
    };

    // Flip the flag — the inner Text's derive must recompute → width changes.
    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__flag, true);"#)
        .unwrap();
    app.render();

    let w2 = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_text_width(&tree, root.id).expect("Text still mounted after flip")
    };
    assert_ne!(
        w1, w2,
        "Text inside ReadableSubscribe must update when its atom changes — both reads were {w1}"
    );
}

/// Variant: the `ReadableSubscribe > Container > Text(derive)` subtree is placed
/// inside a `Stack > Positioned` (mirrors the implicit-animations demo's card
/// structure). If this fails, the Stack/Positioned wrapper is what blocks
/// reactive propagation to the inner Text.
#[test]
fn readable_subscribe_inside_stack_positioned_still_updates() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, derive, Container, Text, ReadableSubscribe, Positioned, Stack, mutate, render } from "tur:std";
        globalThis.__flag = source(false);
        const flag = globalThis.__flag;
        const cardText = derive(function (g) {
            return g.get(flag) ? "EXPANDED_LABEL_LONG" : "short";
        });
        const inner = Container({
            children: [ Text({ text: cardText, fontSize: 16 }) ]
        });
        const rs = ReadableSubscribe({
            readables: [flag],
            onUpdate$: mutate(function () {}),
            child: inner
        });
        const positioned = Positioned({ left: 30, top: 30, child: rs });
        const stack = Stack({ children: [ positioned ] });
        render(stack);
    "#)
    .unwrap();

    app.render();
    let w1 = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_text_width(&tree, root.id).expect("Text should be mounted")
    };

    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__flag, true);"#)
        .unwrap();
    app.render();

    let w2 = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_text_width(&tree, root.id).expect("Text still mounted after flip")
    };
    assert_ne!(
        w1, w2,
        "Text inside Stack>Positioned>ReadableSubscribe must update — both reads were {w1}"
    );
}

/// Reproduces the implicit-animations structure: `ReadableSubscribe` wraps a
/// `Container` that has an ANIMATED prop (width driven by a `progress$`
/// source), an inner reactive `Text(derive(flag))` child, and an `onUpdate$`
/// that retargets + `ctrl.forward()`. If flipping `flag` no longer updates the
/// Text, the animation machinery is what breaks sibling reactive propagation.
#[test]
fn animated_container_pattern_inner_text_still_updates() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, derive, Container, Text, ReadableSubscribe, mutate, set, render } from "tur:std";
        import { createAnimationController } from "tur:animation";

        globalThis.__flag = source(false);
        const flag = globalThis.__flag;

        // Animation progress (drives the Container's width).
        const progress = source(1.0);
        const widthTween = { begin: 120, end: 120, lerp: function (tt) { return this.begin + (this.end - this.begin) * tt; } };

        const cardText = derive(function (g) {
            return g.get(flag) ? "EXPANDED_LABEL_LONG" : "short";
        });

        // The Container's width is animated (reads progress); the Text reads flag.
        const inner = Container({
            width: derive(function (g) { return widthTween.lerp(g.get(progress)); }),
            children: [ Text({ text: cardText, fontSize: 16 }) ]
        });

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function (_sctx, v) { set(progress, v); })
        });

        const tree = ReadableSubscribe({
            readables: [flag],
            onUpdate$: mutate(function () {
                widthTween.begin = widthTween.lerp(1.0);
                widthTween.end = 200;
                ctrl.forward();
            }),
            child: inner
        });
        render(tree);
    "#)
    .unwrap();

    app.render();
    let w1 = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_text_width(&tree, root.id).expect("Text should be mounted")
    };

    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__flag, true);"#)
        .unwrap();
    app.render();
    // advance past the animation duration so retarget + tick have run
    app.advance(std::time::Duration::from_millis(300)).unwrap();
    app.render();

    let w2 = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        find_text_width(&tree, root.id).expect("Text still mounted after flip")
    };
    assert_ne!(
        w1, w2,
        "Text inside the AnimatedContainer pattern must update when flag flips — both reads were {w1}"
    );
}

/// Decisive triple-nesting repro: three nested `ReadableSubscribe`+controller
/// wrappers (mirroring AnimatedPositioned > AnimatedOpacity > AnimatedContainer),
/// each driving its own `progress$`, with the innermost wrapping a
/// `Container > Text(derive(flag))`. If the Text fails to update on flag flip,
/// the triple-nested-controller structure is what breaks propagation.
#[test]
fn triple_nested_readable_subscribe_inner_text_still_updates() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, derive, set, Container, Text, ReadableSubscribe, mutate, render, Opacity } from "tur:std";
        import { createAnimationController } from "tur:animation";
        globalThis.__flag = source(false);
        const flag = globalThis.__flag;
        // Sink captures the cardText derive's recomputed value (ground truth,
        // independent of layout measurement).
        globalThis.__sink = source("INITIAL");

        function makeLayer(child, animatedProp) {
            const progress = source(1.0);
            const ctrl = createAnimationController({
                duration: 200, curve: "linear",
                onTick: mutate(function (_s, v) { set(progress, v); })
            });
            const inner = animatedProp
                ? Container({ width: derive(function (g) { return 100 + 50 * g.get(progress); }), children: [child] })
                : Opacity({ value: derive(function (g) { return 0.5 + 0.5 * g.get(progress); }), child: child });
            return ReadableSubscribe({
                readables: [flag],
                onUpdate$: mutate(function () { /* no-op */ }),
                child: inner
            });
        }

        const cardText = derive(function (g) {
            const v = g.get(flag) ? "EXPANDED" : "compact";
            set(globalThis.__sink, v);
            return v;
        });
        const text = Text({ text: cardText, fontSize: 16 });
        const layer3 = makeLayer(text, true);
        const layer2 = makeLayer(layer3, false);
        const layer1 = makeLayer(layer2, true);
        render(layer1);
    "#)
    .unwrap();

    app.render();
    app.eval_module_source(r#"import { get } from "tur:std"; globalThis.__result = get(globalThis.__sink);"#)
        .unwrap();
    let v1: String = app.eval_js("globalThis.__result");

    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__flag, true);"#)
        .unwrap();
    app.render();
    app.advance(std::time::Duration::from_millis(300)).unwrap();
    app.render();

    app.eval_module_source(r#"import { get } from "tur:std"; globalThis.__result = get(globalThis.__sink);"#)
        .unwrap();
    let v2: String = app.eval_js("globalThis.__result");
    assert_eq!(
        v1, "compact",
        "initial cardText should be 'compact' — got {v1:?}"
    );
    assert_eq!(
        v2, "EXPANDED",
        "cardText inside triple-nested ReadableSubscribe must recompute to 'EXPANDED' after flag flip — got {v2:?}"
    );
}

/// End-to-end implicit-animation test: a JS `AnimatedContainer`-style wrapper
/// (ReadableSubscribe + Tween + AnimationController) must animate its
/// Container's width from the old target to the new target over `duration`
/// when the target source flips. Reads the Container's computed width at
/// t=0 (start), mid, and end to verify the interpolation actually advances.
#[test]
fn js_animated_container_pattern_animates_width_over_time() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, derive, Container, ReadableSubscribe, mutate, set, render } from "tur:std";
        import { createAnimationController } from "tur:animation";
        globalThis.__target = source(100);
        const target = globalThis.__target;
        const progress = source(1.0);
        const widthTween = {
            begin: 100, end: 100,
            lerp: function (tt) { return this.begin + (this.end - this.begin) * tt; }
        };
        const container = Container({
            width: derive(function (g) { return widthTween.lerp(g.get(progress)); })
        });
        const ctrl = createAnimationController({
            duration: 200, curve: "linear",
            onTick: mutate(function (_s, v) { set(progress, v); })
        });
        const tree = ReadableSubscribe({
            readables: [target],
            onUpdate$: mutate(function () {
                widthTween.begin = widthTween.lerp(1.0);
                widthTween.end = 200;
                ctrl.forward();
            }),
            child: container
        });
        render(tree);
    "#)
    .unwrap();

    app.render();
    // Find the Container id once.
    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        // root -> ReadableSubscribe(pass-through) -> Container
        let rs = tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap();
        tree.get_element(ElementNodeId::new(rs.children[0].as_u64())).unwrap().id
    };
    let width_at = |app: &TurTestApp| {
        let tree = app.element_tree();
        tree.get_element(container_id).unwrap().computed_layout.size.width
    };

    // Before retarget: width = lerp(1.0) = end = 100.
    assert_eq!(width_at(&app), 100.0, "initial width should be 100");

    // Flip the target -> on_updated -> retarget (begin=100, end=200) + forward.
    app.eval_module_source(r#"import { set } from "tur:std"; set(globalThis.__target, 200);"#)
        .unwrap();
    app.render();
    // ctrl.forward() resets progress to 0 then advances; after a tiny advance
    // the width should be moving away from 100 toward 200.
    app.advance(std::time::Duration::from_millis(100)).unwrap();
    app.render();
    let mid = width_at(&app);
    assert!(
        mid > 110.0 && mid < 190.0,
        "mid-animation width should be ~150 (got {mid})"
    );

    // Past duration -> progress = 1.0 -> width = end = 200.
    app.advance(std::time::Duration::from_millis(200)).unwrap();
    app.render();
    assert_eq!(width_at(&app), 200.0, "post-animation width should be 200");
}
