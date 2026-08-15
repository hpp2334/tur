//! Multi-view-root behavior: one element tree per view root, independent
//! screens / `viewportSize$`, routed pointer events, host-driven
//! tear-down/setup lifecycle, and the zero-root headless build.

use std::time::Duration;

use tur_engine::core::element::ViewRootId;
use tur_integration_tests::TurTestApp;

/// Two roots mount, lay out under their OWN viewports, and expose
/// independent `viewportSize$` atoms.
#[test]
fn two_roots_mount_and_layout_independently() {
    let app = TurTestApp::new_multi_root(vec![("sidebar", 100.0, 300.0), ("detail", 500.0, 400.0)])
        .unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, Container, Text, createColor } from "tur:std";
        setViewRoot(viewRoot("main"),
            Container({ width: 400, height: 60, color: createColor(0, 255, 0, 255),
                        queryKey: ["main-box"] }));
        setViewRoot(viewRoot("sidebar"),
            Container({ width: 100, height: 50, color: createColor(255, 0, 0, 255),
                        queryKey: ["sidebar-box"] }));
        setViewRoot(viewRoot("detail"),
            Container({ width: 500, height: 80, color: createColor(0, 0, 255, 255),
                        queryKey: ["detail-box"] }));
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // All three roots appear in the merged snapshot, in registration order.
    let tree = app.element_tree();
    assert_eq!(tree.roots.len(), 3, "main + sidebar + detail");
    assert_eq!(tree.roots[0].0, ViewRootId::new(0));
    assert_eq!(tree.roots[1].0, ViewRootId::new(1));
    assert_eq!(tree.roots[2].0, ViewRootId::new(2));

    // Each root's box laid out under its own viewport (RootElement fills
    // the root's tight constraints).
    let root_size = |key: &[&str]| -> (f64, f64) {
        let id = app.query_element(key).expect("query");
        let node = app.dev_tool_get_element(id).expect("node");
        node.size
    };
    let (sw, sh) = root_size(&["sidebar-box"]);
    assert_eq!((sw, sh), (100.0, 50.0));
    let (dw, dh) = root_size(&["detail-box"]);
    assert_eq!((dw, dh), (500.0, 80.0));

    // Per-root size atoms hold each root's viewport, not a global one.
    app.eval_module_source(
        r#"
        import { viewRoot, get } from "tur:std";
        globalThis.__sizes = JSON.stringify([
            get(viewRoot("main").viewportSize$),
            get(viewRoot("sidebar").viewportSize$),
            get(viewRoot("detail").viewportSize$),
        ]);
    "#,
    )
    .unwrap();
    assert_eq!(
        app.eval_js("globalThis.__sizes"),
        r#"[{"width":400,"height":600},{"width":100,"height":300},{"width":500,"height":400}]"#
    );
}

/// Resizing one root updates its own screen + atom only; the other root's
/// tree is untouched.
#[test]
fn resize_one_root_leaves_other_untouched() {
    let app = TurTestApp::new_multi_root(vec![("second", 200.0, 200.0)]).unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, Container, createColor } from "tur:std";
        setViewRoot(viewRoot("main"),
            Container({ width: 100, height: 100, color: createColor(255, 0, 0, 255),
                        queryKey: ["main-box"] }));
        setViewRoot(viewRoot("second"),
            Container({ width: 100, height: 100, color: createColor(0, 255, 0, 255),
                        queryKey: ["second-box"] }));
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Resize ONLY "main".
    app.with_app(|a| a.resize_root("main", 800, 900, 1.0));
    app.wait_for_timeout(Duration::ZERO);

    app.eval_module_source(
        r#"
        import { viewRoot, get } from "tur:std";
        globalThis.__vp = JSON.stringify([
            get(viewRoot("main").viewportSize$),
            get(viewRoot("second").viewportSize$),
        ]);
    "#,
    )
    .unwrap();
    assert_eq!(
        app.eval_js("globalThis.__vp"),
        r#"[{"width":800,"height":900},{"width":200,"height":200}]"#
    );
}

/// Pointer events are routed to the owning root's tree — a click at the
/// same local coordinates only fires the target root's handler.
#[test]
fn pointer_events_route_to_owning_root() {
    let mut app = TurTestApp::new_multi_root(vec![("second", 200.0, 200.0)]).unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, Container, PointerInteract, mutate } from "tur:std";
        globalThis.__hits = [];
        const track = (name) => mutate(() => { globalThis.__hits.push(name); });
        setViewRoot(viewRoot("main"),
            Container({ width: 200, height: 200, children: [PointerInteract({
                onClick: track("main"), child: Container({ width: 200, height: 200 }) })] }));
        setViewRoot(viewRoot("second"),
            Container({ width: 200, height: 200, children: [PointerInteract({
                onClick: track("second"), child: Container({ width: 200, height: 200 }) })] }));
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // (100,100) is inside BOTH roots' boxes in their own local space — but
    // only the "second" root receives the routed click.
    app.click_root("second", 100.0, 100.0);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__hits)"),
        r#"["second"]"#
    );

    app.click_root("main", 100.0, 100.0);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__hits)"),
        r#"["second","main"]"#
    );
}

/// Host `tear_down_root` destroys the root's tree (unmount hooks fire),
/// retains the mount intent, and `setup_root` rebuilds from it (mount hooks
/// fire again). Reactive atoms survive; `active$` mirrors the lifecycle.
#[test]
fn tear_down_and_setup_round_trip() {
    let app = TurTestApp::new_multi_root(vec![("panel", 200.0, 200.0)]).unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, get, mutate, Container, lifecycleView, createColor } from "tur:std";
        globalThis.__events = [];
        const tracked = lifecycleView(() => ({
            element: Container({ width: 50, height: 50, color: createColor(0, 128, 0, 255),
                                 queryKey: ["panel-box"] }),
            onMounted$: mutate(() => globalThis.__events.push("mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("unmounted")),
        }));
        setViewRoot(viewRoot("panel"), tracked);
    "#,
    )
    .unwrap();
    let read_state = || {
        app.eval_module_source(
            r#"
            import { viewRoot, get } from "tur:std";
            globalThis.__state = JSON.stringify({
                active: get(viewRoot("panel").active$),
                events: globalThis.__events,
            });
        "#,
        )
        .unwrap();
        app.wait_for_timeout(Duration::ZERO);
        app.eval_js("globalThis.__state")
    };

    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(read_state(), r#"{"active":true,"events":["mounted"]}"#);

    // Tear down: tree destroyed, unmount fires, intent RETAINED.
    app.with_app(|a| a.tear_down_root("panel"));
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["panel-box"]).is_none(),
        "tree destroyed"
    );
    assert_eq!(
        read_state(),
        r#"{"active":false,"events":["mounted","unmounted"]}"#
    );

    // Set up: rebuilt from the retained intent, mount fires again. The
    // teardown released the target, so a fresh surface attaches.
    app.with_app(|a| {
        a.setup_root(
            "panel",
            Box::new(tur_engine::renderer::noop::NoopSurface),
            (200.0, 200.0),
            1.0,
        )
    })
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["panel-box"]).is_some(), "tree rebuilt");
    assert_eq!(
        read_state(),
        r#"{"active":true,"events":["mounted","unmounted","mounted"]}"#
    );
}

/// `resetViewRoot` destroys the tree AND clears the intent — a later host
/// `setup_root` finds nothing to rebuild.
#[test]
fn reset_clears_mount_intent() {
    let app = TurTestApp::new_multi_root(vec![("panel", 200.0, 200.0)]).unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, mutate, Container, lifecycleView } from "tur:std";
        globalThis.__events = [];
        const tracked = lifecycleView(() => ({
            element: Container({ width: 50, height: 50, queryKey: ["panel-box"] }),
            onMounted$: mutate(() => globalThis.__events.push("mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("unmounted")),
        }));
        setViewRoot(viewRoot("panel"), tracked);
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    app.eval_module_source(
        r#"import { resetViewRoot, viewRoot } from "tur:std"; resetViewRoot(viewRoot("panel"));"#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["panel-box"]).is_none());
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__events)"),
        r#"["mounted","unmounted"]"#
    );

    // Intent cleared: setup finds nothing.
    app.with_app(|a| {
        a.setup_root(
            "panel",
            Box::new(tur_engine::renderer::noop::NoopSurface),
            (200.0, 200.0),
            1.0,
        )
    })
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["panel-box"]).is_none());
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__events)"),
        r#"["mounted","unmounted"]"#
    );
}

/// Replacing a root's view via `setViewRoot` unmounts the old subtree
/// before building the new one.
#[test]
fn set_view_root_replaces_previous_tree() {
    let app = TurTestApp::new_multi_root(vec![("panel", 200.0, 200.0)]).unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, mutate, Container, lifecycleView } from "tur:std";
        globalThis.__events = [];
        const first = lifecycleView(() => ({
            element: Container({ width: 50, height: 50, queryKey: ["first-box"] }),
            onMounted$: mutate(() => globalThis.__events.push("first:mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("first:unmounted")),
        }));
        const second = lifecycleView(() => ({
            element: Container({ width: 60, height: 60, queryKey: ["second-box"] }),
            onMounted$: mutate(() => globalThis.__events.push("second:mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("second:unmounted")),
        }));
        setViewRoot(viewRoot("panel"), first);
        globalThis.__second = second;
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["first-box"]).is_some());

    app.eval_module_source(
        r#"import { setViewRoot, viewRoot } from "tur:std"; setViewRoot(viewRoot("panel"), globalThis.__second);"#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        app.query_element(&["first-box"]).is_none(),
        "old subtree gone"
    );
    assert!(
        app.query_element(&["second-box"]).is_some(),
        "new subtree built"
    );
    // Hook order within one flush: the lifecycle pass drains `on_mounted`
    // before `before_destroy`, so the new view's mount hook precedes the
    // old view's unmount hook (same ordering as a Switch branch swap).
    // Assert as a set.
    let events = app.eval_js("JSON.stringify(globalThis.__events.sort())");
    assert_eq!(
        events,
        r#"["first:mounted","first:unmounted","second:mounted"]"#
    );
}

/// `setViewRoot` while the root is torn down records the intent only — the
/// build is deferred until the host sets the root up.
#[test]
fn set_view_root_while_torn_down_defers_build() {
    let app = TurTestApp::new_multi_root(vec![("panel", 200.0, 200.0)]).unwrap();
    app.with_app(|a| a.tear_down_root("panel"));
    app.wait_for_timeout(Duration::ZERO);

    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, Container } from "tur:std";
        setViewRoot(viewRoot("panel"),
            Container({ width: 50, height: 50, queryKey: ["panel-box"] }));
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["panel-box"]).is_none(),
        "deferred while torn down"
    );

    app.with_app(|a| {
        a.setup_root(
            "panel",
            Box::new(tur_engine::renderer::noop::NoopSurface),
            (200.0, 200.0),
            1.0,
        )
    })
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["panel-box"]).is_some(),
        "built on setup"
    );
}

/// A zero-view-root build is headless: the engine runs JS, the reactive
/// store works, `viewRoots()` reports no roots.
#[test]
fn zero_root_headless_build_runs_js_only() {
    let driver = tur_integration_tests::TestSchedulerDriver::new();
    let runtime = tur_engine::TurRuntime::builder()
        .scheduler(driver.clone())
        .font_loader(std::sync::Arc::new(tur_native::NativeFontLoader::new()))
        .clock(std::sync::Arc::new(
            tur_integration_tests::MutexFixedClock::new(0),
        ))
        .plugin(tur_engine::TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .build()
        .unwrap();
    let app = runtime
        .app_builder()
        .renderer(Box::new(tur_engine::renderer::NoopRenderer::new()))
        .build()
        .unwrap();
    let looper = tur_integration_tests::RawAppLooper::new(app.clone(), driver);

    futures::executor::block_on(app.eval_module(
        r#"
        import { source, get, viewRoots } from "tur:std";
        globalThis.__val = get(source(41)) + 1;
        globalThis.__roots = JSON.stringify(viewRoots());
    "#,
    ))
    .unwrap();
    looper.wait_for_timeout(Duration::ZERO);

    let eval = |src: &str| futures::executor::block_on(app.backend().eval_js(src));
    assert_eq!(eval("globalThis.__val"), "42");
    assert_eq!(eval("globalThis.__roots"), "[]");
}
/// Node ids are composite `{ view_root_id, node_id }`: each tree's counter
/// restarts at 1, the root field makes ids unique instance-wide, and the
/// shared subscriber graph keys on the composite (not the bare counter) so
/// same-numbered nodes in different roots never collide.
#[test]
fn node_ids_are_root_qualified_and_counters_restart_per_tree() {
    let app = TurTestApp::new_multi_root(vec![("sidebar", 400.0, 300.0), ("detail", 500.0, 400.0)])
        .unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, Text, source, derive, set } from "tur:std";
        const mainCount$ = source(1);
        const sideCount$ = source(1);
        globalThis.__setMain = (v) => set(mainCount$, v);
        globalThis.__setSide = (v) => set(sideCount$, v);
        setViewRoot(viewRoot("main"),
            Text({ text: derive((ctx) => "m".repeat(ctx.get(mainCount$))) }));
        setViewRoot(viewRoot("sidebar"),
            Text({ text: derive((ctx) => "s".repeat(ctx.get(sideCount$))) }));
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Both roots' first nodes share local numbering (the counter restarts
    // per tree) but carry different roots — composite ids never collide.
    let tree = app.element_tree();
    // Only mounted roots appear (main + sidebar; "detail" has no view).
    assert_eq!(tree.roots.len(), 2);
    let (main_root, main_first) = tree.roots[0];
    let (side_root, side_first) = tree.roots[1];
    assert!(
        main_first.node() > 0 && side_first.node() > 0,
        "numbering starts after the temp-parent alloc"
    );
    assert_eq!(
        main_first.node(),
        side_first.node(),
        "each tree's counter restarts — same local numbering"
    );
    assert_eq!(main_first.root(), main_root);
    assert_eq!(side_first.root(), side_root);
    assert_ne!(main_first, side_first);

    // Subscriber graph keys on the composite: setting BOTH roots' atoms
    // grows BOTH texts (if the bare counters collided, the second
    // `subscribe` would overwrite the first edge and one text would stay
    // single-character).
    let text_width = |root: tur_engine::core::element::ViewRootId| -> f64 {
        let first = tree
            .roots
            .iter()
            .find(|(r, _)| *r == root)
            .map(|(_, id)| *id)
            .unwrap();
        // RootElement wrapper's first child is the Text.
        let text_id = app.dev_tool_get_element(first.into()).unwrap().children[0];
        app.dev_tool_get_element(text_id).unwrap().size.0
    };
    let (w_main_before, w_side_before) = (text_width(main_root), text_width(side_root));
    app.eval_js("globalThis.__setMain(12); globalThis.__setSide(14);");
    app.wait_for_timeout(Duration::ZERO);
    let (w_main_after, w_side_after) = (text_width(main_root), text_width(side_root));
    assert!(
        w_main_after > w_main_before * 4.0,
        "main text re-laid-out on its own atom ({w_main_before} -> {w_main_after})"
    );
    assert!(
        w_side_after > w_side_before * 4.0,
        "sidebar text re-laid-out on its own atom ({w_side_before} -> {w_side_after})"
    );

    // Dev tool ids cross JS as `{ root, node }` objects and route by root:
    // `{root:1,node:1}` resolves sidebar's first node, `{root:9,node:1}`
    // resolves nothing.
    let side_name = app.eval_js("turDevTool.getElement({ root: 1, node: 2 }) ? 'ok' : 'missing'");
    assert_eq!(side_name, "ok");
    let bad =
        app.eval_js("turDevTool.getElement({ root: 9, node: 2 }) === null ? 'null' : 'found'");
    assert_eq!(bad, "null");
}

/// Deferred-surface lifecycle test doubles: a spy renderer / target /
/// surface set that records `create_target` calls, target drops, image
/// uploads, and renders, so tests can observe the main-side attach /
/// release / replay behavior.
mod spy {
    use std::cell::RefCell;
    use std::rc::Rc;

    use tur_engine::core::image_resource::{ImageResource, ImageResourceId};
    use tur_engine::core::render::{RenderCommand, RenderTarget, Renderer, Surface, SurfaceHandle};
    use tur_engine::error::TurError;

    /// Opaque surface payload carrying an integer tag so a re-attach with a
    /// fresh surface is distinguishable from the first attach.
    pub struct SpySurface(pub u32);
    impl Surface for SpySurface {
        fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
            self
        }
    }

    #[derive(Default)]
    pub struct SpyLog {
        /// `(surface_tag, viewport, dpr)` per `create_target` call.
        pub created: Vec<(u32, (f64, f64), f64)>,
        /// Surface tags of dropped targets, in drop order.
        pub dropped_tags: Vec<u32>,
        /// Every `upload_image_resource` (initial uploads + replays).
        pub uploads: Vec<ImageResourceId>,
        /// `render_commands` invocation count.
        pub renders: usize,
    }

    struct SpyTarget {
        tag: u32,
        log: Rc<RefCell<SpyLog>>,
    }
    impl RenderTarget for SpyTarget {
        fn render_commands(&mut self, _commands: &[RenderCommand]) {
            self.log.borrow_mut().renders += 1;
        }
        fn upload_image_resource(&mut self, id: ImageResourceId, _image: &ImageResource) {
            self.log.borrow_mut().uploads.push(id);
        }
    }
    impl Drop for SpyTarget {
        fn drop(&mut self) {
            self.log.borrow_mut().dropped_tags.push(self.tag);
        }
    }

    pub struct SpyRenderer {
        log: Rc<RefCell<SpyLog>>,
    }
    impl Renderer for SpyRenderer {
        fn create_target(
            &mut self,
            surface: SurfaceHandle,
            viewport: (f64, f64),
            dpr: f64,
        ) -> Result<Box<dyn RenderTarget>, TurError> {
            let any: Box<dyn std::any::Any> = surface.into_any();
            let surface = *any
                .downcast::<SpySurface>()
                .map_err(|_| TurError::Other("spy renderer expects a SpySurface".into()))?;
            self.log
                .borrow_mut()
                .created
                .push((surface.0, viewport, dpr));
            Ok(Box::new(SpyTarget {
                tag: surface.0,
                log: self.log.clone(),
            }))
        }
    }

    pub fn spy() -> (SpyRenderer, Rc<RefCell<SpyLog>>) {
        let log = Rc::new(RefCell::new(SpyLog::default()));
        (SpyRenderer { log: log.clone() }, log)
    }
}

/// A root declared at build is PENDING: no render target exists,
/// `setViewRoot` records intent only, `active$` is false, and `resize_root`
/// still tracks the worker screen. `setup_root` with the surface attaches
/// (target created fail-fast), builds the tree, flips `active$`, and the
/// next frame renders into the fresh target.
#[test]
fn pending_root_defers_build_until_surface_attach() {
    let (renderer, log) = spy::spy();
    let app =
        TurTestApp::new_pending_multi_root(vec![("panel", 200.0, 200.0)], Some(Box::new(renderer)))
            .unwrap();

    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, get, mutate, Container, lifecycleView, createColor } from "tur:std";
        globalThis.__events = [];
        const tracked = lifecycleView(() => ({
            element: Container({ width: 50, height: 50, color: createColor(0, 128, 0, 255),
                                 queryKey: ["panel-box"] }),
            onMounted$: mutate(() => globalThis.__events.push("mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("unmounted")),
        }));
        setViewRoot(viewRoot("panel"), tracked);
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(
        app.query_element(&["panel-box"]).is_none(),
        "deferred: no surface yet"
    );
    assert_eq!(app.eval_js("JSON.stringify(globalThis.__events)"), r#"[]"#);
    assert!(log.borrow().created.is_empty(), "no target before attach");
    assert_eq!(log.borrow().renders, 0, "nothing rendered");

    // resize_root on a pending root still updates the worker screen.
    app.with_app(|a| a.resize_root("panel", 320, 240, 2.0));
    app.wait_for_timeout(Duration::ZERO);
    app.eval_module_source(
        r#"
        import { viewRoot, get } from "tur:std";
        globalThis.__vp = JSON.stringify({
            size: get(viewRoot("panel").viewportSize$),
            active: get(viewRoot("panel").active$),
        });
    "#,
    )
    .unwrap();
    assert_eq!(
        app.eval_js("globalThis.__vp"),
        r#"{"size":{"width":320,"height":240},"active":false}"#
    );

    // Attach: target created with the passed size, tree built, active$ set.
    app.with_app(|a| a.setup_root("panel", Box::new(spy::SpySurface(7)), (320.0, 240.0), 2.0))
        .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        log.borrow().created,
        vec![(7, (320.0, 240.0), 2.0)],
        "target created at attach with the passed size"
    );
    assert!(
        app.query_element(&["panel-box"]).is_some(),
        "built on attach"
    );
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__events)"),
        r#"["mounted"]"#
    );
    app.eval_module_source(
        r#"
        import { viewRoot, get } from "tur:std";
        globalThis.__active = JSON.stringify(get(viewRoot("panel").active$));
    "#,
    )
    .unwrap();
    assert_eq!(app.eval_js("globalThis.__active"), "true");
    assert!(
        log.borrow().renders > 0,
        "frame rendered into the fresh target"
    );
}

/// `tear_down_root` releases the root's render target (frees the GPU/GL
/// resources for a gone canvas) while retaining the mount intent; a later
/// `setup_root` with a FRESH surface creates a new target and rebuilds the
/// tree from the intent.
#[test]
fn teardown_releases_surface_and_reattach_rebuilds_from_intent() {
    let (renderer, log) = spy::spy();
    let app =
        TurTestApp::new_pending_multi_root(vec![("panel", 200.0, 200.0)], Some(Box::new(renderer)))
            .unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, mutate, Container, lifecycleView } from "tur:std";
        globalThis.__events = [];
        const tracked = lifecycleView(() => ({
            element: Container({ width: 50, height: 50, queryKey: ["panel-box"] }),
            onMounted$: mutate(() => globalThis.__events.push("mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("unmounted")),
        }));
        setViewRoot(viewRoot("panel"), tracked);
    "#,
    )
    .unwrap();
    app.with_app(|a| a.setup_root("panel", Box::new(spy::SpySurface(1)), (200.0, 200.0), 1.0))
        .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["panel-box"]).is_some());

    // Teardown: target released, tree destroyed, intent retained.
    app.with_app(|a| a.tear_down_root("panel"));
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(log.borrow().dropped_tags, vec![1], "target released");
    assert!(
        app.query_element(&["panel-box"]).is_none(),
        "tree destroyed"
    );
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__events)"),
        r#"["mounted","unmounted"]"#
    );

    // Replace the intent while torn down — still deferred.
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, get, mutate, Container, lifecycleView, createColor } from "tur:std";
        const second = lifecycleView(() => ({
            element: Container({ width: 60, height: 60, color: createColor(0, 0, 255, 255),
                                 queryKey: ["panel-box-2"] }),
            onMounted$: mutate(() => globalThis.__events.push("mounted")),
            beforeDestroy$: mutate(() => globalThis.__events.push("unmounted")),
        }));
        setViewRoot(viewRoot("panel"), second);
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert!(app.query_element(&["panel-box-2"]).is_none());

    // Re-attach with a fresh surface: new target, rebuilt from the intent.
    app.with_app(|a| a.setup_root("panel", Box::new(spy::SpySurface(2)), (200.0, 200.0), 1.0))
        .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        log.borrow().created,
        vec![(1, (200.0, 200.0), 1.0), (2, (200.0, 200.0), 1.0)]
    );
    assert!(
        app.query_element(&["panel-box-2"]).is_some(),
        "rebuilt from the retained intent"
    );
    assert_eq!(
        app.eval_js("JSON.stringify(globalThis.__events)"),
        r#"["mounted","unmounted","mounted"]"#
    );
    assert!(log.borrow().renders > 0, "renders into the new target");
}

/// Re-attaching replays every retained image resource into the fresh
/// target (the context-loss / canvas-replacement path): main retains the
/// pixel blobs across teardown, and `setup_root` re-uploads them.
#[test]
fn reattach_replays_retained_image_resources() {
    let (renderer, log) = spy::spy();
    let app = TurTestApp::new_pending_multi_root(vec![], Some(Box::new(renderer))).unwrap();
    app.with_app(|a| a.setup_root("main", Box::new(spy::SpySurface(1)), (400.0, 600.0), 1.0))
        .unwrap();
    app.eval_module_source(
        r#"
        import { createImageResource, Container } from "tur:std";
        const pngBytes = new Uint8Array([
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
            0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120,
            218, 99, 252, 255, 159, 161, 30, 0, 7, 130, 2, 127, 61, 200, 72, 239, 0, 0,
            0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ]);
        createImageResource(pngBytes);
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let count = app.with_app(|a| a.backend().image_resource_count());
    assert_eq!(count, 1, "image shipped to main");
    assert_eq!(log.borrow().uploads.len(), 1, "initial upload");
    let image_id = log.borrow().uploads[0];

    // Teardown drops the target; the pixel blob stays retained on main.
    app.with_app(|a| a.tear_down_root("main"));
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.with_app(|a| a.backend().image_resource_count()),
        1,
        "blob retained across teardown"
    );

    // Re-attach: the retained resource is replayed into the fresh target.
    app.with_app(|a| a.setup_root("main", Box::new(spy::SpySurface(2)), (400.0, 600.0), 1.0))
        .unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        log.borrow().uploads,
        vec![image_id, image_id],
        "replay upload into the fresh target"
    );
}

/// `setup_root` fails fast on an unknown root name and on a mismatched
/// surface/renderer pairing (the surface is consumed, nothing attaches).
#[test]
fn setup_root_fails_fast_on_unknown_name_and_mismatched_surface() {
    let (renderer, _log) = spy::spy();
    let app = TurTestApp::new_pending_multi_root(vec![], Some(Box::new(renderer))).unwrap();

    let unknown =
        app.with_app(|a| a.setup_root("nope", Box::new(spy::SpySurface(1)), (10.0, 10.0), 1.0));
    assert!(unknown.is_err(), "unknown root name errors");

    // The default harness renderer is NoopRenderer — it rejects a
    // non-NoopSurface at attach time (fail-fast on mismatched pairing).
    let app2 = TurTestApp::new_pending_multi_root(vec![], None).unwrap();
    let mismatched =
        app2.with_app(|a| a.setup_root("main", Box::new(spy::SpySurface(1)), (10.0, 10.0), 1.0));
    assert!(mismatched.is_err(), "mismatched surface pairing errors");
}

/// Every `PlatformEvent` is a `Shell(ShellEvent { view_root_id, payload })`.
///
/// Root-routing semantics per payload kind:
/// - **Pointer / Wheel / Resize** — routed to `view_root_id`'s tree: a wheel
///   stamped with root B's id scrolls only B's scroll view, never A's.
/// - **Key / Ime / Custom** — the engine performs NO root routing (hosts
///   gate shell focus themselves and only dispatch for focused shells): a
///   Key/Ime pair stamped with root B's id still reaches the editable
///   focused in root A.
#[test]
fn shell_events_carry_view_root_for_every_payload() {
    use tur_engine::builtin_plugins::scroll::ScrollViewElement;
    use tur_engine::builtin_plugins::text::elements::EditableTextElement;
    use tur_engine::core::platform::{ImeEvent, PlatformEvent, ShellEventPayload};

    let mut app = TurTestApp::new_multi_root(vec![("second", 400.0, 300.0)]).unwrap();
    app.eval_module_source(
        r#"
        import { setViewRoot, viewRoot, Input, ScrollView, Container } from "tur:std";
        setViewRoot(viewRoot("main"),
            Input({ queryKey: ["main-input"], width: 200, height: 40 }));
        setViewRoot(viewRoot("second"),
            ScrollView({ queryKey: ["second-scroll"], child: Container({ width: 100, height: 900 }) }));
    "#,
    )
    .unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let second_root = app.root_id("second").expect("second root");

    // Locate main's editable under its queryKey'd wrapper.
    let editable_id = {
        let wrapper = app.query_element(&["main-input"]).expect("query");
        let wrapper = wrapper.as_element_id();
        let tree = app.element_tree();
        let wrapper_node = tree.get_element(wrapper).unwrap();
        tree.get_element(wrapper_node.children[0].as_element_id())
            .unwrap()
            .id
    };
    let text_of = |app: &TurTestApp| -> String {
        app.with_element(editable_id, |e| {
            e.cast::<EditableTextElement>()
                .map(|el| el.text().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default()
    };

    // Focus main's editable by clicking it inside main's tree.
    let (cx, cy) = app
        .get_element_absolute_bounds(editable_id)
        .unwrap()
        .center();
    app.click_root("main", cx, cy);
    app.wait_for_timeout(Duration::ZERO);

    // Key + Ime stamped with SECOND root's id reach the editable focused
    // in MAIN — the engine does not root-route these payload kinds.
    app.with_app(|a| {
        a.push_platform_event(PlatformEvent::shell(
            second_root,
            ShellEventPayload::Key(tur_engine::core::platform::KeyEvent {
                key: "a".into(),
                code: "KeyA".into(),
                modifiers: Default::default(),
                event_type: tur_engine::core::platform::KeyEventType::Down,
            }),
        ));
    });
    app.with_app(|a| {
        a.push_platform_event(PlatformEvent::shell(
            second_root,
            ShellEventPayload::Ime(ImeEvent::CompositionStart),
        ));
        a.push_platform_event(PlatformEvent::shell(
            second_root,
            ShellEventPayload::Ime(ImeEvent::CompositionEnd { text: "写".into() }),
        ));
    });
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(text_of(&app), "a写", "Key/Ime ignore the stamped root");

    // Wheel stamped with SECOND root's id scrolls only second's scroll
    // view. (Main has no scroll view; "only B" is pinned by the offset
    // landing on second's element.)
    app.with_app(|a| {
        a.push_platform_event(PlatformEvent::shell(
            second_root,
            ShellEventPayload::Wheel {
                delta_x: 0.0,
                delta_y: 60.0,
                position: tur_engine::core::layout::Offset::new(50.0, 50.0),
            },
        ));
    });
    app.wait_for_timeout(Duration::ZERO);
    let scroll_id = {
        let wrapper = app.query_element(&["second-scroll"]).expect("scroll query");
        wrapper.as_element_id()
    };
    let second_offset = app
        .with_element(scroll_id, |e| {
            e.cast::<ScrollViewElement>()
                .map(|sv| sv.scroll_offset())
                .unwrap_or(-1.0)
        })
        .unwrap_or(-1.0);
    assert_eq!(second_offset, 60.0, "wheel routed to second's tree");
    assert_eq!(text_of(&app), "a写", "main untouched by second's wheel");
}
