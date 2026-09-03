//! Virtual apps — `VirtualAppView` hosting complete nested engine instances
//! (own worker, realm, store, tree). Pins: rendering (child ops replayed
//! into the parent's batch from the host element's paint), realm/store
//! isolation, destroy+construct lifecycle, layout-driven resize, lazy
//! controllers, module-error surfacing, shared-vsync animation, image id
//! re-keying, and the child-as-`TurApp` facade.

use std::cell::RefCell;
use std::rc::Rc;

use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::layout::{MouseButton, Offset};
use tur_engine::core::platform::{PointerDeviceKind, PointerInput};
use tur_engine::core::render::{RenderCommand, RenderCommandBatch, Renderer};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::core::shell::ShellEvent;
use tur_engine::core::virtual_app::VirtualAppId;
use tur_integration_tests::MutexFixedClock;
use tur_integration_tests::TestSchedulerDriver;
use tur_integration_tests::{RawAppLooper, TestShell};
use tur_native::NativeFontLoader;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Host-side renderer that records the last applied batch (structural
/// assertions without a GPU — the child's ops must appear in the PARENT's
/// batch, replayed by the host element's paint).
#[derive(Clone, Default)]
struct BatchRecorder(Rc<RefCell<RenderCommandBatch>>);

impl Renderer for BatchRecorder {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        *self.0.borrow_mut() = commands.to_vec();
    }
}

impl BatchRecorder {
    fn contains_text_layout(&self) -> bool {
        self.0.borrow().iter().any(|c| {
            matches!(c, RenderCommand::Paint { ops, .. } if ops
                .iter()
                .any(|op| matches!(op, tur_engine::core::render::CanvasOp::FillTextLayout { .. })))
        })
    }

    fn contains_draw_image(&self) -> bool {
        self.0.borrow().iter().any(|c| {
            matches!(c, RenderCommand::Paint { ops, .. } if ops
                .iter()
                .any(|op| matches!(op, tur_engine::core::render::CanvasOp::DrawImage { .. })))
        })
    }

    /// Whether the last batch contains a solid-fill rect of the given
    /// (r, g, b) whose width matches `width` (±0.5) — pins size-dependent
    /// child repaints (a full-viewport child fill replays at the child's
    /// current viewport width).
    fn contains_solid_rect(&self, rgb: (u8, u8, u8), width: f64) -> bool {
        use tur_engine::core::layout::Geometry;
        use tur_engine::core::render::CanvasOp;
        use tur_engine::core::render::brush::Brush;
        self.0.borrow().iter().any(|c| {
            matches!(c, RenderCommand::Paint { ops, .. } if ops.iter().any(|op| {
                matches!(
                    op,
                    CanvasOp::FillGeometry {
                        geometry: Geometry::Rect(size),
                        brush: Brush::SolidColor(color),
                        ..
                    }
                    if (color.r(), color.g(), color.b()) == rgb
                        && (size.width - width).abs() <= 0.5
                )
            }))
        })
    }
}

fn build_runtime() -> (Rc<TurRuntime>, Rc<TestSchedulerDriver>, WorkerPoolHandle) {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("test", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .plugin(tur_animation::TurAnimationPlugin)
        .build()
        .expect("runtime build");
    (runtime, driver, pool)
}

/// Build a parent instance whose host-side renderer records batches.
fn build_parent(
    runtime: &Rc<TurRuntime>,
    driver: &Rc<TestSchedulerDriver>,
    pool: &WorkerPoolHandle,
    recorder: BatchRecorder,
) -> (Rc<tur_engine::TurApp>, RawAppLooper) {
    let (app, looper) = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .renderer(Box::new(recorder), (400.0, 300.0), 1.0)
        .shell(TestShell::new(driver.vsync_source()))
        .build()
        .expect("parent app");
    let looper = RawAppLooper::new(app.clone(), looper, driver.clone());
    (app, looper)
}

fn eval_js(app: &Rc<tur_engine::TurApp>, source: &str) -> String {
    futures::executor::block_on(app.eval_js(source))
}

/// Push a full click (down + up, same `time_ms` → classified a single tap)
/// into an app's platform queue, in PARENT viewport coordinates.
fn click_at(app: &Rc<tur_engine::TurApp>, x: f64, y: f64, time_ms: u64) {
    app.push_platform_event(ShellEvent::Pointer(PointerInput::PointerDown {
        position: Offset::new(x, y),
        button: MouseButton::Left,
        time_ms,
        device: PointerDeviceKind::Mouse,
    }));
    app.push_platform_event(ShellEvent::Pointer(PointerInput::PointerUp {
        position: Offset::new(x, y),
        button: MouseButton::Left,
        time_ms,
        device: PointerDeviceKind::Mouse,
    }));
}

/// Quote a JS source so it can be embedded as a string literal inside a
/// parent module (the parent registers it via `createModuleSource`).
fn js_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The parent-side module: a `VirtualAppView` bound to a reactive
/// controller slot, plus test hooks on `globalThis` (`__make` / `__spawn` /
/// `__spawnB` / `__unspawn` / `__destroy` / `__get`). `child_b` optionally
/// registers a second source (`__spawnB`) for recreate tests.
fn parent_module(child_src: &str) -> String {
    parent_module_b(child_src, None, false)
}

/// Same module, but the controller keeps its child alive across unbinds —
/// the playground's viewer-churn shape (tab switches unmount the host
/// element while the child keeps running).
fn parent_module_keep_alive(child_src: &str) -> String {
    parent_module_b(child_src, None, true)
}

fn parent_module_b(child_a: &str, child_b: Option<&str>, keep_alive: bool) -> String {
    let ka = if keep_alive { ", keepAlive: true" } else { "" };
    let spawn_b = match child_b {
        Some(b) => format!(
            r#"
            globalThis.__spawnB = () => {{
                app = createVirtualAppController({{ source: createModuleSource({child}){ka} }});
                globalThis.__app = app;
                store.set(app$$, app);
            }};
            "#,
            child = js_quote(b),
            ka = ka,
        ),
        None => "globalThis.__spawnB = undefined;".to_string(),
    };
    format!(
        r#"
        import {{
            Text, VirtualAppView, createModuleSource, createVirtualAppController,
            mount, source, view,
        }} from "tur:std";

        const app$$ = source(null);
        let app = null;

        export function start({{ store }}) {{
            globalThis.__make = () => {{
                app = createVirtualAppController({{ source: createModuleSource({child}){ka} }});
                globalThis.__app = app;
                return true;
            }};
            globalThis.__spawn = () => {{ globalThis.__make(); store.set(app$$, app); }};
            {spawn_b}
            globalThis.__unspawn = () => {{ store.set(app$$, null); }};
            globalThis.__destroy = () => {{ if (app) store.set(app.destroy$); }};
            globalThis.__get = (a) => store.get(a);
            mount(view(() => VirtualAppView({{ app$$: app$$ }})));
        }}
    "#,
        child = js_quote(child_a),
        spawn_b = spawn_b,
        ka = ka,
    )
    .replace("app$$", "app$")
}

/// The one and only child controller (first hosted virtual app).
fn only_child(app: &Rc<tur_engine::TurApp>) -> Option<Rc<tur_engine::TurApp>> {
    app.virtual_apps().into_iter().next()
}

fn wait_status(looper: &RawAppLooper, app: &Rc<tur_engine::TurApp>, want: &str) -> bool {
    let ok = looper.wait_for(|| eval_js(app, "globalThis.__get(globalThis.__app.status$)") == want);
    if !ok {
        eprintln!(
            "[wait_status] wanted {want:?}, got {:?} (err={:?})",
            eval_js(app, "globalThis.__get(globalThis.__app.status$)"),
            eval_js(app, "globalThis.__get(globalThis.__app.errorMsg$)"),
        );
    }
    ok
}

// ---------------------------------------------------------------------------
// Child sources
// ---------------------------------------------------------------------------

const CHILD_TEXT: &str = r#"
    import { Text, mount, view } from "tur:std";
    export function start() {
        globalThis.__who = "A";
        mount(view(() => Text({ text: "hello-from-child" })));
        return () => { globalThis.__cleaned = "A"; };
    }
"#;

const CHILD_B: &str = r#"
    import { Text, mount, view } from "tur:std";
    export function start() {
        globalThis.__who = "B";
        mount(view(() => Text({ text: "hello-from-child-b" })));
    }
"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The core vertical slice: a bound controller spawns a child in the
/// `"virtual"` pool, the child's module runs (real `mount`), and the child's
/// paint is replayed into the PARENT's batch by the host element's paint.
#[test]
fn virtual_app_renders_nested_module() {
    let (runtime, driver, pool) = build_runtime();
    let recorder = BatchRecorder::default();
    let (app, looper) = build_parent(&runtime, &driver, &pool, recorder.clone());

    futures::executor::block_on(app.load_module(parent_module(CHILD_TEXT).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(
        wait_status(&looper, &app, "running"),
        "child should reach status running, got: {}",
        eval_js(&app, "globalThis.__get(globalThis.__app.status$)")
    );

    // The child is hosted (one virtual app) and its module really ran.
    let child = only_child(&app).expect("one hosted child");
    assert_eq!(eval_js(&child, "globalThis.__who"), "A");

    // The child's text paint is replayed into the parent's batch.
    assert!(
        looper.wait_for(|| recorder.contains_text_layout()),
        "parent batch should contain the child's FillTextLayout op"
    );
}

/// Realm isolation both ways: the child's globals are invisible to the
/// parent and vice versa.
#[test]
fn virtual_app_isolates_realms() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    futures::executor::block_on(app.load_module(parent_module(CHILD_TEXT).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__parent = 'P'");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));

    let child = only_child(&app).expect("one hosted child");
    assert_eq!(eval_js(&child, "globalThis.__who"), "A");
    // Parent can't see the child's global…
    assert_eq!(eval_js(&app, "globalThis.__who"), "undefined");
    // …and the child can't see the parent's.
    assert_eq!(eval_js(&child, "globalThis.__parent"), "undefined");
}

/// Controllers are lazy declarations — nothing spawns until an element
/// binds; first bind spawns.
#[test]
fn virtual_app_controller_is_lazy() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    futures::executor::block_on(app.load_module(parent_module(CHILD_TEXT).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__make()");
    looper.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        eval_js(&app, "globalThis.__get(globalThis.__app.status$)"),
        "idle",
        "unbound controller stays idle"
    );
    assert!(
        app.virtual_apps().is_empty(),
        "unbound controller must not spawn a child"
    );

    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    assert_eq!(app.virtual_apps().len(), 1);
}

/// Recompile semantics: `destroy$` + a new controller (new source) +
/// rebinding. The old child is destroyed under its token; the new one runs.
#[test]
fn virtual_app_recreate_runs_new_source() {
    let (runtime, driver, pool) = build_runtime();
    let recorder = BatchRecorder::default();
    let (app, looper) = build_parent(&runtime, &driver, &pool, recorder.clone());

    futures::executor::block_on(
        app.load_module(parent_module_b(CHILD_TEXT, Some(CHILD_B), false).as_str()),
    )
    .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    let first_id = app.virtual_apps()[0].id();

    // destroy$ → destroyed (child gone under the old token) …
    eval_js(&app, "globalThis.__destroy()");
    assert!(wait_status(&looper, &app, "destroyed"));
    assert!(
        app.virtual_apps().iter().all(|a| a.id() != first_id),
        "destroyed child leaves the host"
    );

    // … then a new controller with the new source binds and runs.
    eval_js(&app, "globalThis.__spawnB()");
    assert!(wait_status(&looper, &app, "running"));
    let child = only_child(&app).expect("new child");
    assert_eq!(eval_js(&child, "globalThis.__who"), "B");
    assert!(
        looper.wait_for(|| recorder.contains_text_layout()),
        "the new child's content should render after recreate"
    );
}

/// Unbinding (`app$` → null) destroys the child (default `keepAlive:
/// false`); rebinding respawns it.
#[test]
fn virtual_app_destroy_on_unbind() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    futures::executor::block_on(app.load_module(parent_module(CHILD_TEXT).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    assert_eq!(app.virtual_apps().len(), 1);

    eval_js(&app, "globalThis.__unspawn()");
    assert!(wait_status(&looper, &app, "destroyed"));
    // The child leaves `virtual_apps()` once the looper routes the
    // `Destroy` control host-side (the status flips on the worker first).
    assert!(
        looper.wait_for(|| app.virtual_apps().is_empty()),
        "unbinding must destroy the child"
    );

    // Rebind respawns (fresh incarnation under the same controller).
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    assert_eq!(app.virtual_apps().len(), 1);
    let child = only_child(&app).unwrap();
    assert_eq!(eval_js(&child, "globalThis.__who"), "A");
}

/// Layout-driven resize: the host element's rect is forwarded to the
/// child's `viewportSize$`.
#[test]
fn virtual_app_resize_follows_layout() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let child_src = r#"
        import { Text, mount, view, viewportSize$ } from "tur:std";
        export function start({ store }) {
            globalThis.__vpW = () => store.get(viewportSize$).width;
            globalThis.__vpH = () => store.get(viewportSize$).height;
            mount(view(() => Text({ text: "v" })));
        }
    "#;
    futures::executor::block_on(app.load_module(parent_module(child_src).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    let child = only_child(&app).expect("child");

    // The element fills the root viewport — initial 400×300 …
    assert!(
        looper.wait_for(|| eval_js(&child, "globalThis.__vpW()") == "400"),
        "child viewport should track the element rect (initial 400), got: {}",
        eval_js(&child, "globalThis.__vpW()")
    );

    // …and a parent resize relayouts the element → Resize control → child.
    app.resize(600, 400, 1.0);
    assert!(
        looper.wait_for(|| eval_js(&child, "globalThis.__vpW()") == "600"),
        "child viewport should follow the parent resize, got: {}",
        eval_js(&child, "globalThis.__vpW()")
    );
    assert_eq!(eval_js(&child, "globalThis.__vpH()"), "400");
}

/// A keep-alive rebind after the parent viewport changed must repaint the
/// child at its NEW rect — with no vsync tick to rescue it. The child's
/// fresh frame reaches the parent as an engine `AppEvent` while the parent
/// is otherwise idle; if that event doesn't drive its own flush, the
/// parent keeps replaying the child's STALE (old-viewport) frame forever
/// (the playground's mobile-tab viewer: unmount → viewport change →
/// remount showed the desktop-sized frame clipped into the new element).
#[test]
fn virtual_app_rebind_after_resize_repaints_without_vsync() {
    let (runtime, driver, pool) = build_runtime();
    let recorder = BatchRecorder::default();
    let (app, looper) = build_parent(&runtime, &driver, &pool, recorder.clone());

    // The child paints a full-viewport magenta fill: the replayed rect's
    // width IS the child's viewport width.
    let child_src = r##"
        import { Color, Container, derive, mount, view, viewportSize$ } from "tur:std";
        export function start({ store }) {
            globalThis.__vpW = () => store.get(viewportSize$).width;
            mount(view(() => Container({
                color: Color.hex("#ff00ff"),
                width: derive((ctx) => ctx.get(viewportSize$).width),
                height: 24,
            })));
        }
    "##;
    let parent_mod = parent_module_keep_alive(child_src);
    futures::executor::block_on(app.load_module(parent_mod.as_str())).expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(
        wait_status(&looper, &app, "running"),
        "child should reach running, got: {}",
        eval_js(&app, "globalThis.__get(globalThis.__app.status$)")
    );

    // The initial 400-wide replay is present (parent viewport 400×300).
    assert!(
        looper.wait_for(|| recorder.contains_solid_rect((255, 0, 255), 400.0)),
        "initial child fill should replay at the element width (400)"
    );

    // Unbind (keep-alive: the child survives), change the parent
    // viewport, rebind — the keep-alive rebind path.
    eval_js(&app, "globalThis.__unspawn()");
    app.resize(600, 350, 1.0);
    eval_js(&app, "globalThis.__spawn()");

    // ONE vsync tick — enough to pump the child's `Resize` platform
    // event through (platform events ride the frame loop by design) —
    // then settle in real time WITHOUT any further parent pumps: the
    // child's fresh frame arrives as an `AppEvent` at an idle parent and
    // must be consumed by the event itself, not a later tick.
    driver.fire_vsync();
    let mut settled = false;
    for _ in 0..25 {
        // Sleep in real time ON the driven LocalSet: the engine's loopers
        // are spawn_local tasks here, so a woken looper (e.g. by the
        // worker shipping a batch) runs during the wait. This pumps
        // nothing itself — no vsync ticks fire.
        driver.block_on(async { tokio::time::sleep(std::time::Duration::from_millis(20)).await });
        if recorder.contains_solid_rect((255, 0, 255), 600.0) {
            settled = true;
            break;
        }
    }
    assert!(
        settled,
        "rebind must replay the child at the new width (600) with no further vsync tick; \
         600-fill present: {}, 400-fill present: {}",
        recorder.contains_solid_rect((255, 0, 255), 600.0),
        recorder.contains_solid_rect((255, 0, 255), 400.0),
    );
}

/// A broken child source surfaces as `status$ = "error"` with detail, and
/// the parent tree stays intact.
#[test]
fn virtual_app_module_error_surfaces() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let broken = "export function start( { this is not valid javascript";
    futures::executor::block_on(app.load_module(parent_module(broken).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "error"));
    assert!(
        !eval_js(&app, "globalThis.__get(globalThis.__app.errorMsg$)").is_empty(),
        "module error detail should surface on errorMsg$"
    );
    // The parent realm is unaffected.
    assert_eq!(eval_js(&app, "1 + 1"), "2");
}

/// The child shares the parent's vsync source — an animation in the child
/// ticks while the parent pumps (fire fans out to every subscriber).
#[test]
fn virtual_app_child_animation_ticks() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let child_src = r#"
        import { createAnimationController } from "tur:animation";
        import { Text, mount, mutate, view } from "tur:std";
        let ticks = 0;
        export function start({ store }) {
            globalThis.__ticks = () => ticks;
            const ctrl = createAnimationController({
                duration: 100000,
                repeat: "infinite",
                onTick: mutate(() => { ticks++; }),
            });
            ctrl.forward();
            mount(view(() => Text({ text: "animating" })));
        }
    "#;
    futures::executor::block_on(app.load_module(parent_module(child_src).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    let child = only_child(&app).expect("child");

    assert!(
        looper.wait_for(|| eval_js(&child, "globalThis.__ticks()") != "0"),
        "child animation should tick on the shared vsync, got: {}",
        eval_js(&child, "globalThis.__ticks()")
    );
}

/// Child images re-key into the parent's id space: the parent's batch
/// contains a `DrawImage` op and the parent host retains the upload.
#[test]
fn virtual_app_child_images_render() {
    let (runtime, driver, pool) = build_runtime();
    let recorder = BatchRecorder::default();
    let (app, looper) = build_parent(&runtime, &driver, &pool, recorder.clone());

    let child_src = r#"
        import { Image, createImageResource, mount, view } from "tur:std";
        const pngBytes = new Uint8Array([
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
            0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120,
            218, 99, 252, 255, 159, 161, 30, 0, 7, 130, 2, 127, 61, 200, 72, 239, 0, 0,
            0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ]);
        const resource = createImageResource(pngBytes);
        export function start() {
            mount(view(() => Image({ resourceId: resource, width: 100, height: 100 })));
        }
    "#;
    futures::executor::block_on(app.load_module(parent_module(child_src).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));

    assert!(
        looper.wait_for(|| recorder.contains_draw_image()),
        "parent batch should contain the child's DrawImage op (re-keyed id)"
    );
    assert!(
        app.image_resource_count() >= 1,
        "the parent host retains the re-keyed child image upload"
    );
}

/// Children surface as ordinary `TurApp`s — driving a child through the
/// facade (a second `load_module`) honors the module lifecycle: the first
/// module's cleanup runs, the second goes live.
#[test]
fn virtual_app_child_module_lifecycle_via_facade() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    futures::executor::block_on(app.load_module(parent_module(CHILD_TEXT).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    let child = only_child(&app).expect("child");
    assert_eq!(eval_js(&child, "globalThis.__cleaned"), "undefined");

    // Parse-first reload on the child through the ordinary facade.
    futures::executor::block_on(child.load_module(CHILD_B)).expect("reload child");
    assert_eq!(
        eval_js(&child, "globalThis.__cleaned"),
        "A",
        "the first module's cleanup must run before the reload evaluates"
    );
    assert_eq!(eval_js(&child, "globalThis.__who"), "B");
}

/// Parent destroy tears down hosted children (each child's module cleanup
/// runs in its own worker).
#[test]
fn virtual_app_parent_destroy_tears_down_children() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    futures::executor::block_on(app.load_module(parent_module(CHILD_TEXT).as_str()))
        .expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    assert_eq!(app.virtual_apps().len(), 1);

    // Identity model: the parent is embedder-hosted (ROOT); a hosted child
    // carries its parent-minted incarnation token — never ROOT.
    assert_eq!(app.id(), VirtualAppId::ROOT);
    let child = only_child(&app).expect("one hosted child");
    assert_ne!(child.id(), VirtualAppId::ROOT);

    // Parent destroy clears the hosted-children map synchronously on the
    // host thread (each child's teardown message is fire-and-forget).
    app.destroy();
    assert!(
        app.virtual_apps().is_empty(),
        "parent destroy must tear down hosted children"
    );
}

// ---------------------------------------------------------------------------
// Worker pool handles (`forWorkerPool`)
// ---------------------------------------------------------------------------

/// `forWorkerPool` resolves against the runtime's registered pools eagerly
/// at the call site — an unknown name throws a TypeError right there (not
/// an async status error after spawn).
#[test]
fn for_worker_pool_unknown_name_throws_eagerly() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let parent = r#"
        import { forWorkerPool } from "tur:std";
        export function start() {
            globalThis.__poolErr = (name) => {
                try { forWorkerPool(name); return "no-throw"; }
                catch (e) { return e.message; }
            };
        }
    "#;
    futures::executor::block_on(app.load_module(parent)).expect("load parent");

    let msg = eval_js(&app, r#"globalThis.__poolErr("nope")"#);
    assert!(
        msg.contains("unknown worker pool") && msg.contains("nope"),
        "expected an eager unknown-pool TypeError, got: {msg:?}"
    );
    // The registered pools are listed to guide the caller.
    assert!(
        msg.contains("test") && msg.contains("virtual"),
        "got: {msg:?}"
    );

    // A registered name resolves without throwing.
    assert_eq!(eval_js(&app, r#"globalThis.__poolErr("test")"#), "no-throw");
    let _ = looper; // not driven in this test
}

/// A controller created with `forWorkerPool("test")` spawns its child into
/// that named pool (the real handle path — resolve by name, spawn by
/// handle).
#[test]
fn for_worker_pool_named_pool_spawns_child() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let parent = format!(
        r#"
        import {{
            VirtualAppView, createModuleSource, createVirtualAppController,
            forWorkerPool, mount, source, view,
        }} from "tur:std";

        const app$ = source(null);

        export function start({{ store }}) {{
            globalThis.__spawn = () => {{
                const app = createVirtualAppController({{
                    source: createModuleSource({child}),
                    pool: forWorkerPool("test"),
                }});
                globalThis.__app = app;
                store.set(app$, app);
            }};
            globalThis.__get = (a) => store.get(a);
            mount(view(() => VirtualAppView({{ app$: app$ }})));
        }}
    "#,
        child = js_quote(CHILD_TEXT),
    );
    futures::executor::block_on(app.load_module(parent.as_str())).expect("load parent");

    eval_js(&app, "globalThis.__spawn()");
    assert!(
        wait_status(&looper, &app, "running"),
        "child should spawn into the named `test` pool, got: {} (err={:?})",
        eval_js(&app, "globalThis.__get(globalThis.__app.status$)"),
        eval_js(&app, "globalThis.__get(globalThis.__app.errorMsg$)"),
    );
    let child = only_child(&app).expect("one hosted child");
    assert_eq!(eval_js(&child, "globalThis.__who"), "A");
}

/// The controller's `pool` option is handle-only: a raw string is rejected
/// with a pointer at `forWorkerPool`.
#[test]
fn controller_pool_option_rejects_raw_strings() {
    let (runtime, driver, pool) = build_runtime();
    let (app, _looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let parent = r#"
        import { createModuleSource, createVirtualAppController } from "tur:std";
        export function start() {
            globalThis.__badPool = () => {
                try {
                    createVirtualAppController({
                        source: createModuleSource("export function start() {}"),
                        pool: "test",
                    });
                    return "no-throw";
                } catch (e) { return e.message; }
            };
        }
    "#;
    futures::executor::block_on(app.load_module(parent)).expect("load parent");

    let msg = eval_js(&app, "globalThis.__badPool()");
    assert!(
        msg.contains("forWorkerPool"),
        "`pool` must demand a handle from forWorkerPool(), got: {msg:?}"
    );
}

/// Pointer events over the host element forward into the hosted child,
/// translated into child-local coordinates (position − host rect origin);
/// the child composes gestures in its own arena. Clicks OUTSIDE the host
/// (in the parent's padding) never reach the child.
#[test]
fn virtual_app_forwards_pointer_events_to_child() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let child = r##"
        import { Color, Container, mutate, mount, PointerInteract, view } from "tur:std";
        export function start() {
            globalThis.__clicks = 0;
            globalThis.__local = "none";
            mount(view(() =>
                PointerInteract({
                    onClick: mutate((_ctx, ev) => {
                        globalThis.__clicks += 1;
                        globalThis.__local =
                            ev && ev.local ? (ev.local.x + "," + ev.local.y) : "none";
                    }),
                    // Fill the whole child viewport (300x200 — the host
                    // element's rect) so any forwarded click hits.
                    child: Container({
                        width: 300,
                        height: 200,
                        color: Color.hex("#6366f1"),
                    }),
                }),
            ));
        }
    "##;
    let parent = format!(
        r#"
        import {{
            Container, VirtualAppView, createModuleSource,
            createVirtualAppController, mount, source, view,
        }} from "tur:std";

        const app$ = source(null);

        export function start({{ store }}) {{
            globalThis.__spawn = () => {{
                const app = createVirtualAppController({{
                    source: createModuleSource({child}),
                }});
                globalThis.__app = app;
                store.set(app$, app);
            }};
            globalThis.__get = (a) => store.get(a);
            // 50px padding: the host element sits at (50, 50) in the
            // 400x300 parent viewport, sized 300x200.
            mount(view(() => Container({{
                padding: 50,
                children: [view(() => VirtualAppView({{ app$: app$ }}))],
            }})));
        }}
    "#,
        child = js_quote(child),
    );
    futures::executor::block_on(app.load_module(parent.as_str())).expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    let child_app = only_child(&app).expect("one hosted child");

    // Click inside the host rect (host at (50,50), size 300x200) → the
    // child composes the click at CHILD-LOCAL (70, 30).
    click_at(&app, 120.0, 80.0, 1_000);
    assert!(
        looper.wait_for(|| eval_js(&child_app, "String(globalThis.__clicks)") == "1"),
        "click inside the host must forward into the child (clicks={:?}, local={:?})",
        eval_js(&child_app, "String(globalThis.__clicks)"),
        eval_js(&child_app, "globalThis.__local"),
    );
    assert_eq!(
        eval_js(&child_app, "globalThis.__local"),
        "70,30",
        "the child must see child-local coordinates (position − host origin)"
    );

    // Click OUTSIDE the host (inside the parent's 50px padding): the child
    // must NOT see it.
    click_at(&app, 10.0, 10.0, 5_000);
    let _ = looper.wait_for_timeout(std::time::Duration::from_millis(200));
    assert_eq!(
        eval_js(&child_app, "String(globalThis.__clicks)"),
        "1",
        "clicks outside the host rect must not reach the child"
    );
}

/// Forwarded clicks are exact under rapid scripted tapping: N taps at one
/// point compose exactly N child clicks (no drops, no double-applies), and
/// alternating taps on two disjoint targets apply to the right one each
/// time (no misattribution).
#[test]
fn virtual_app_forwarded_clicks_are_exact() {
    let (runtime, driver, pool) = build_runtime();
    let (app, looper) = build_parent(&runtime, &driver, &pool, BatchRecorder::default());

    let child = r##"
        import { Color, Container, mutate, mount, PointerInteract, Row, view } from "tur:std";
        export function start() {
            globalThis.__a = 0;
            globalThis.__b = 0;
            const bump = (which) => mutate(() => {
                globalThis[which] += 1;
            });
            mount(view(() =>
                Row({
                    children: [
                        PointerInteract({
                            onClick: bump("__a"),
                            child: Container({ width: 150, height: 200, color: Color.hex("#6366f1") }),
                        }),
                        PointerInteract({
                            onClick: bump("__b"),
                            child: Container({ width: 150, height: 200, color: Color.hex("#ec4899") }),
                        }),
                    ],
                }),
            ));
        }
    "##;
    let parent = format!(
        r#"
        import {{
            Container, VirtualAppView, createModuleSource,
            createVirtualAppController, mount, source, view,
        }} from "tur:std";

        const app$ = source(null);

        export function start({{ store }}) {{
            globalThis.__spawn = () => {{
                const app = createVirtualAppController({{
                    source: createModuleSource({child}),
                }});
                globalThis.__app = app;
                store.set(app$, app);
            }};
            globalThis.__get = (a) => store.get(a);
            // 50px padding: host rect (50,50,300,200). Child-local x<150 =
            // button A, x>=150 = button B.
            mount(view(() => Container({{
                padding: 50,
                children: [view(() => VirtualAppView({{ app$: app$ }}))],
            }})));
        }}
    "#,
        child = js_quote(child),
    );
    futures::executor::block_on(app.load_module(parent.as_str())).expect("load parent");
    eval_js(&app, "globalThis.__spawn()");
    assert!(wait_status(&looper, &app, "running"));
    let child_app = only_child(&app).expect("one hosted child");

    // 12 rapid taps on button A (child-local (75,100) → parent (125,150)).
    // Distinct, sub-double-click-window-free timestamps (500ms apart).
    for i in 0..12 {
        click_at(&app, 125.0, 150.0, 10_000 + i * 500);
    }
    assert!(
        looper.wait_for(|| eval_js(&child_app, "String(globalThis.__a)") == "12"),
        "12 taps must compose exactly 12 clicks (a={:?}, b={:?})",
        eval_js(&child_app, "String(globalThis.__a)"),
        eval_js(&child_app, "String(globalThis.__b)"),
    );
    assert_eq!(eval_js(&child_app, "String(globalThis.__b)"), "0");

    // Alternate A/B — each tap must land on its own target.
    for i in 0..8 {
        let (x, t) = if i % 2 == 0 {
            (225.0, 20_000 + i * 500) // B (child-local 175,100)
        } else {
            (125.0, 20_000 + i * 500) // A
        };
        click_at(&app, x, 150.0, t);
    }
    assert!(
        looper.wait_for(|| eval_js(&child_app, "String(globalThis.__b)") == "4"),
        "alternating taps must land on their own targets (a={:?}, b={:?})",
        eval_js(&child_app, "String(globalThis.__a)"),
        eval_js(&child_app, "String(globalThis.__b)"),
    );
    assert!(
        looper.wait_for(|| eval_js(&child_app, "String(globalThis.__a)") == "16"),
        "alternating taps must all land (a={:?})",
        eval_js(&child_app, "String(globalThis.__a)"),
    );
}
