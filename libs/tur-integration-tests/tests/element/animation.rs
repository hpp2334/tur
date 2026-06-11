use std::time::Duration;

use tur_integration_tests::TurTestApp;

#[test]
fn animation_controller_forward_with_on_tick() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, container, "width", 100);
        globalThis.__tur.appendChild(ctx, root, container);

        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 200,
            curve: "linear",
            onTick: function(value) {
                var w = 100 + (200 - 100) * value;
                globalThis.__tur.setAttribute(ctx, container, "width", w);
            }
        });
        ctrl.forward();
    "#);

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        tree.get(root.children[0]).unwrap().id
    };

    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        assert_eq!(node.computed_layout.size.width, 100.0,
            "at t=0 width should still be 100");
    }

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w > 110.0 && w < 190.0,
            "at t=100ms (halfway) width should be ~150, got {w}");
    }

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "after duration elapsed width should be 200, got {w}");
    }
}

#[test]
fn animation_controller_reverse_with_on_tick() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, container, "width", 200);
        globalThis.__tur.appendChild(ctx, root, container);

        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 200,
            curve: "linear",
            onTick: function(value) {
                var w = 100 + (200 - 100) * value;
                globalThis.__tur.setAttribute(ctx, container, "width", w);
            }
        });
        ctrl.reverse();
    "#);

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        tree.get(root.children[0]).unwrap().id
    };

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w > 110.0 && w < 190.0,
            "reverse halfway: width should be ~150, got {w}");
    }

    app.advance(Duration::from_millis(150)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        assert_eq!(node.computed_layout.size.width, 100.0,
            "reverse complete: width should be 100");
    }
}

#[test]
fn animation_controller_stop_freezes_value() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, container, "width", 100);
        globalThis.__tur.appendChild(ctx, root, container);

        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 200,
            curve: "linear",
            onTick: function(value) {
                var w = 100 + (200 - 100) * value;
                globalThis.__tur.setAttribute(ctx, container, "width", w);
            }
        });
        ctrl.forward();
        globalThis.__test_ctrl = ctrl;
    "#);

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        tree.get(root.children[0]).unwrap().id
    };

    app.advance(Duration::from_millis(50)).unwrap();
    app.render();
    let frozen_width = {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
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
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, frozen_width,
            "after stop + advance, width should stay frozen at {frozen_width}, got {w}");
    }
}

#[test]
fn animation_controller_repeats() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, container, "width", 100);
        globalThis.__tur.appendChild(ctx, root, container);

        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 100,
            curve: "linear",
            onTick: function(value) {
                var w = 100 + (200 - 100) * value;
                globalThis.__tur.setAttribute(ctx, container, "width", w);
            }
        });
        ctrl.repeat(3);
        ctrl.forward();
    "#);

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        tree.get(root.children[0]).unwrap().id
    };

    app.advance(Duration::from_millis(250)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w > 140.0 && w < 160.0,
            "after 250ms (2.5x100ms), halfway through 3rd repeat, got {w}");
    }

    app.advance(Duration::from_millis(100)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert_eq!(w, 200.0,
            "after 350ms (past 3x100ms), width should be 200 (completed), got {w}");
    }
}

#[test]
fn animation_controller_status_transitions() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.appendChild(ctx, root, container);

        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 100
        });
        globalThis.__test_ctrl = ctrl;

        globalThis.__statuses = [ctrl.status];
        ctrl.forward();
        globalThis.__statuses.push(ctrl.status);
    "#);

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
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.appendChild(ctx, root, container);

        globalThis.__ended = false;
        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 100,
            onEnd: function() {
                globalThis.__ended = true;
            }
        });
        ctrl.forward();
    "#);

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
    app.eval_js(r#"
        var ctx = globalThis.__tur.__ctx;
        var root = globalThis.__tur.createRoot(ctx);
        var container = globalThis.__tur.createContainer(ctx);
        globalThis.__tur.setAttribute(ctx, container, "width", 0);
        globalThis.__tur.appendChild(ctx, root, container);

        var ctrl = globalThis.__tur.createAnimationController(ctx, {
            duration: 1000,
            curve: "easeIn",
            onTick: function(value) {
                globalThis.__tur.setAttribute(ctx, container, "width", 1000 * value);
            }
        });
        ctrl.forward();
    "#);

    let container_id = {
        let tree = app.element_tree();
        let root = tree.root().unwrap();
        tree.get(root.children[0]).unwrap().id
    };

    app.advance(Duration::from_millis(500)).unwrap();
    app.render();
    {
        let tree = app.element_tree();
        let node = tree.get(container_id).unwrap();
        let w = node.computed_layout.size.width;
        assert!(w < 500.0,
            "easeIn at t=0.5: width should be < 500 (slow start), got {w}");
    }
}
