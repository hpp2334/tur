use std::time::Duration;
use tur_engine::core::element::ElementNodeId;

use tur_integration_tests::TurTestApp;

#[test]
fn animation_controller_forward_with_on_tick() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(100);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.forward();
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        assert_eq!(node.computed_layout.size.width, 100.0,
            "at t=0 width should still be 100");
    }

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w > 110.0 && w < 190.0,
            "at t=100ms (halfway) width should be ~150, got {w}");
    }

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "after duration elapsed width should be 200, got {w}");
    }
}

#[test]
fn animation_controller_reverse_with_on_tick() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(200);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.reverse();
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w > 110.0 && w < 190.0,
            "reverse halfway: width should be ~150, got {w}");
    }

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        assert_eq!(node.computed_layout.size.width, 100.0,
            "reverse complete: width should be 100");
    }
}

#[test]
fn animation_controller_stop_freezes_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(100);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.forward();
        globalThis.__test_ctrl = ctrl;
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    app.advance(Duration::from_millis(50)).unwrap();
    app.render();
    let frozen_width = {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        node.computed_layout.size.width
    };
    assert!(frozen_width > 100.0 && frozen_width < 200.0,
        "width should be mid-animation, got {frozen_width}");

    app.eval_js(r#"
        globalThis.__test_ctrl.stop();
    "#);

    app.advance(Duration::from_millis(200)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, frozen_width,
            "after stop + advance, width should stay frozen at {frozen_width}, got {w}");
    }
}

#[test]
fn animation_controller_repeats() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(100);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 100,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.repeat(3);
        ctrl.forward();
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    app.advance(Duration::from_millis(250)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w > 140.0 && w < 160.0,
            "after 250ms (2.5x100ms), halfway through 3rd repeat, got {w}");
    }

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "after 350ms (past 3x100ms), width should be 200 (completed), got {w}");
    }
}

#[test]
fn animation_controller_status_transitions() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const container = Container({});
        render(container);

        const ctrl = createAnimationController({
            duration: 100
        });
        globalThis.__test_ctrl = ctrl;

        globalThis.__statuses = [ctrl.status];
        ctrl.forward();
        globalThis.__statuses.push(ctrl.status);
    "#)
    .unwrap();

    let statuses_raw = app.eval_js(r#"
        globalThis.__statuses[0] + "," + globalThis.__statuses[1];
    "#);
    let statuses: Vec<&str> = statuses_raw.split(',').collect();
    assert_eq!(statuses[0], "stopped");
    assert_eq!(statuses[1], "forward");

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();

    let status: String = app.eval_js(r#"
        globalThis.__test_ctrl.status;
    "#);
    assert_eq!(status, "completed",
        "after duration elapsed, status should be completed, got {status}");
}

#[test]
fn animation_controller_on_end_callback() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const container = Container({});
        render(container);

        globalThis.__ended = false;
        const ctrl = createAnimationController({
            duration: 100,
            onEnd: mutate(function(_sctx) {
                globalThis.__ended = true;
            })
        });
        ctrl.forward();
    "#)
    .unwrap();

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();

    let ended: String = app.eval_js(r#"
        globalThis.__ended ? "true" : "false";
    "#);
    assert_eq!(ended, "true", "onEnd should have been called");
}

#[test]
fn animation_controller_ease_in_curve() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(0);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 1000,
            curve: "easeIn",
            onTick: mutate(function(_sctx, v) {
                set(width$, 1000 * v);
            })
        });
        ctrl.forward();
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    app.advance(Duration::from_millis(500)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w < 500.0,
            "easeIn at t=0.5: width should be < 500 (slow start), got {w}");
    }
}

#[test]
fn animation_controller_pause_freezes_and_resume_continues() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(100);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.forward();
        globalThis.__test_ctrl = ctrl;
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    // Halfway through (100ms of 200ms) → ~150, then pause.
    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    app.eval_js(r#"globalThis.__test_ctrl.pause();"#);

    let paused_width = {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        node.computed_layout.size.width
    };
    assert!(paused_width > 140.0 && paused_width < 160.0,
        "paused width should be ~150, got {paused_width}");

    // Advance 200ms while paused → no movement.
    app.advance(Duration::from_millis(200)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!((w - paused_width).abs() < 1.0,
            "during pause width should stay at {paused_width}, got {w}");
    }

    // Resume — should finish the remaining ~half over ~100ms.
    let status: String = app.eval_js(r#"globalThis.__test_ctrl.status"#);
    assert_eq!(status, "paused", "status should be 'paused'");
    app.eval_js(r#"globalThis.__test_ctrl.resume();"#);
    let status: String = app.eval_js(r#"globalThis.__test_ctrl.status"#);
    assert_eq!(status, "forward", "after resume status should be 'forward'");

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "after resume + advance, width should be 200 (completed), got {w}");
    }
}

#[test]
fn animation_controller_seek_jumps_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(100);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.forward();
        globalThis.__test_ctrl = ctrl;
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    // Jump to 80% immediately.
    app.eval_js(r#"globalThis.__test_ctrl.seek(0.8);"#);
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!((w - 180.0).abs() < 1.0,
            "after seek(0.8) width should be ~180, got {w}");
    }

    // Continue forward from 0.8; with 200ms duration, the remaining 20% takes
    // 40ms. After 60ms the animation should have completed.
    app.advance(Duration::from_millis(60)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "after seek + advance past remaining duration, width should be 200, got {w}");
    }
}

#[test]
fn animation_controller_set_speed_scales_time() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { source, Container, render, mutate, set } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const width$ = source(100);
        const container = Container({ width: width$ });
        render(container);

        const ctrl = createAnimationController({
            duration: 200,
            curve: "linear",
            onTick: mutate(function(_sctx, v) {
                const w = 100 + (200 - 100) * v;
                set(width$, w);
            })
        });
        ctrl.forward();
        globalThis.__test_ctrl = ctrl;
    "#)
    .unwrap();

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root_element().unwrap();
        tree.get_element(ElementNodeId::new(root.children[0].as_u64())).unwrap().id
    };

    // Double speed: 200ms duration should complete in ~100ms.
    app.eval_js(r#"globalThis.__test_ctrl.setSpeed(2.0);"#);

    app.advance(Duration::from_millis(110)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "at 2x speed, 200ms animation should complete after ~110ms, got {w}");
    }

    // Reverse at half speed: should be roughly halfway after 200ms.
    app.eval_js(r#"
        globalThis.__test_ctrl.reverse();
        globalThis.__test_ctrl.setSpeed(0.5);
    "#);
    app.advance(Duration::from_millis(200)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get_element(container_id).unwrap();
        let w = node.computed_layout.size.width;
        // 200ms at 0.5x = 100ms effective of 200ms duration = 50% of the way
        // back from 200 → 150.
        assert!(w > 140.0 && w < 160.0,
            "at 0.5x speed reverse, after 200ms width should be ~150, got {w}");
    }
}

// ---------------------------------------------------------------------------
// Regression tests for the mutation-queue-based callback dispatch.
//
// Before the refactor, animation callbacks (onTick / onEnd) were fired
// synchronously while the engine held a `RefMut<AnimationController>` —
// any JS callback that accessed the controller (e.g. reading `ctrl.status`)
// triggered a boa `BorrowError`. These tests verify callbacks can safely
// read controller state.
// ---------------------------------------------------------------------------

#[test]
fn controller_on_tick_can_read_status_from_forward() {
    // onTick callback reads ctrl.status. Before the fix this would panic
    // with BorrowError because forward() fires onTick(0) synchronously
    // while holding the RefMut.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const container = Container({});
        render(container);

        globalThis.__tick_status = null;
        const ctrl = createAnimationController({
            duration: 200,
            onTick: mutate(function(_ctx, v) {
                globalThis.__tick_status = ctrl.status;
            })
        });
        ctrl.forward();
    "#)
    .unwrap();

    // After forward() + flush, the queued onTick should have fired and read
    // ctrl.status without panicking.
    app.render();

    let status: String = app.eval_js(r#"globalThis.__tick_status"#);
    assert_eq!(status, "forward",
        "onTick should be able to read ctrl.status='forward' without panic, got {status}");
}

#[test]
fn controller_on_end_can_read_status_after_complete() {
    // onEnd callback reads ctrl.status after the animation completes.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const container = Container({});
        render(container);

        globalThis.__end_status = null;
        const ctrl = createAnimationController({
            duration: 50,
            onEnd: mutate(function(_ctx) {
                globalThis.__end_status = ctrl.status;
            })
        });
        ctrl.forward();
    "#)
    .unwrap();

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();

    let status: String = app.eval_js(r#"globalThis.__end_status"#);
    assert_eq!(status, "completed",
        "onEnd should be able to read ctrl.status='completed' without panic, got {status}");
}

#[test]
fn controller_on_tick_can_read_value_during_forward() {
    // onTick callback reads ctrl.value (the controller's own field) during
    // a forward animation. Before the fix, this triggered a `BorrowError`
    // because the read attempted a downcast_ref while forward()'s
    // downcast_mut was still held.
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        const container = Container({});
        render(container);

        globalThis.__tick_values = [];
        const ctrl = createAnimationController({
            duration: 200,
            onTick: mutate(function(_ctx, v) {
                // Read both the eased arg and the controller's own `value`
                // field — both must work without panic.
                globalThis.__tick_values.push(v + "_" + ctrl.value);
            })
        });
        ctrl.forward();
    "#)
    .unwrap();

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();

    let values: String = app.eval_js(r#"globalThis.__tick_values.join("|")"#);
    assert!(!values.is_empty(),
        "onTick should have fired at least once and read ctrl.value without panic, got empty");
    // Every entry should be "X_X" (eased === value for linear curve). The
    // key assertion is that we got here at all without a BorrowError panic.
    for entry in values.split('|') {
        let parts: Vec<&str> = entry.split('_').collect();
        assert_eq!(parts.len(), 2, "expected 'eased_value' format, got {entry:?}");
    }
}

// ---------------------------------------------------------------------------
// Infinite-repeat mode (regression test for the user-requested feature).
// ---------------------------------------------------------------------------

#[test]
fn controller_infinite_does_not_complete_after_many_iterations() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        globalThis.__tick_count = 0;
        globalThis.__end_count = 0;
        globalThis.__last_tick_value = -1;
        const container = Container({});
        render(container);

        globalThis.__ctrl = createAnimationController({
            duration: 100,
            curve: "linear",
            repeat: "infinite",
            onTick: mutate(function(_sctx, v) {
                globalThis.__tick_count++;
                globalThis.__last_tick_value = v;
            }),
            onEnd: mutate(function(_sctx) {
                globalThis.__end_count++;
            })
        });
        globalThis.__ctrl.forward();
    "#)
    .unwrap();

    // Advance well beyond a single iteration — call advance multiple times
    // so the flush loop fires `onTick` each time. 5 calls of 100ms each =
    // 5 iterations worth of animation time.
    for _ in 0..5 {
        app.advance(Duration::from_millis(100)).unwrap();
        app.render();
    }

    let status: String = app.eval_js(r#"String(globalThis.__ctrl.status)"#);
    assert_eq!(status, "forward",
        "infinite animation should still be running after 5 iterations, status = {status:?}");

    let end_count: i64 = app.eval_js(r#"Number(globalThis.__end_count)"#).parse().unwrap_or(0);
    assert_eq!(end_count, 0,
        "onEnd should never fire for an infinite animation, got {end_count}");

    let tick_count: i64 = app.eval_js(r#"Number(globalThis.__tick_count)"#).parse().unwrap_or(0);
    assert!(tick_count >= 5,
        "onTick should fire at least once per flush, got {tick_count}");

    let last: f64 = app.eval_js(r#"Number(globalThis.__last_tick_value)"#).parse().unwrap_or(-1.0);
    assert!((0.0..=1.0).contains(&last),
        "value should always be within [0, 1], got {last}");

    // The tick count should be > 1 (we ran multiple flushes), proving the
    // animation is still ticking and not frozen.
}

#[test]
fn controller_infinite_reverse_cycles_back_to_zero() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        globalThis.__tick_values = [];
        const container = Container({});
        render(container);

        globalThis.__ctrl = createAnimationController({
            duration: 100,
            curve: "linear",
            repeat: "infinite",
            onTick: mutate(function(_sctx, v) {
                globalThis.__tick_values.push(v);
            })
        });
        globalThis.__ctrl.reverse();
    "#)
    .unwrap();

    // Reverse: value goes 1.0 → 0.0, then loops back to 1.0 → 0.0...
    app.advance(Duration::from_millis(250)).unwrap();
    app.render();

    let values: String = app.eval_js(r#"globalThis.__tick_values.join("|")"#);
    let parsed: Vec<f64> = values
        .split('|')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f64>().unwrap_or(-1.0))
        .collect();
    assert!(parsed.len() >= 3,
        "should have at least 3 ticks across multiple iterations, got {parsed:?}");

    // Reverse mode starts at 1.0. The first tick should be close to 1.0.
    assert!(parsed[0] > 0.8,
        "reverse mode should start near 1.0, got {}", parsed[0]);

    // After many iterations, the value should still be in [0, 1].
    let last = parsed[parsed.len() - 1];
    assert!((0.0..=1.0).contains(&last),
        "value should stay in [0, 1] across iterations, got {last}");

    let status: String = app.eval_js(r#"String(globalThis.__ctrl.status)"#);
    assert_eq!(status, "reverse",
        "infinite reverse should still be running, status = {status:?}");
}

#[test]
fn controller_repeat_three_then_completes() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_module_source(r#"
        import { Container, render, mutate } from "tur:std";
        import { createAnimationController } from "tur:animation";
        globalThis.__end_count = 0;
        const container = Container({});
        render(container);

        globalThis.__ctrl = createAnimationController({
            duration: 100,
            curve: "linear",
            repeat: 3,
            onEnd: mutate(function(_sctx) {
                globalThis.__end_count++;
            })
        });
        globalThis.__ctrl.forward();
    "#)
    .unwrap();

    // 3 iterations of 100ms = 300ms total. Advance just past.
    app.advance(Duration::from_millis(320)).unwrap();
    app.render();

    let status: String = app.eval_js(r#"String(globalThis.__ctrl.status)"#);
    assert_eq!(status, "completed",
        "after 3 iterations, finite animation should be completed, status = {status:?}");

    let end_count: i64 = app.eval_js(r#"Number(globalThis.__end_count)"#).parse().unwrap_or(0);
    assert_eq!(end_count, 1,
        "onEnd should fire exactly once when the finite repeat count is reached, got {end_count}");
}
