//! Benchmark scenes. Each loads a representative fixture, drives `frames`
//! frames through the production loop (one pump per frame, one minimal
//! mutation per frame where the scene wants steady change), and prints a
//! per-frame summary read off the frame-stats probe.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn stat(app: &TurTestApp, expression: &str) -> f64 {
    let raw = app.eval_js(&format!("String({expression})"));
    raw.trim()
        .parse()
        .unwrap_or_else(|_| panic!("{expression} = {raw:?} (not a number)"))
}

fn stat_str(app: &TurTestApp, expression: &str) -> String {
    app.eval_js(&format!("String({expression})"))
        .trim()
        .to_string()
}

/// Drive `frames` steady-state frames (16ms virtual step + one pump each)
/// and print the per-frame worker-side summary from `frameStats()`.
///
/// `warmup` flushes are driven first and excluded by reading the cumulative
/// totals before the loop.
fn drive_and_report(app: &TurTestApp, label: &str, frames: usize) {
    // Quiesce the initial mount so the loop measures steady-state frames.
    app.wait_for_timeout(Duration::ZERO);

    let pre = (
        stat(app, "turDevTool.frameStats().flushes"),
        stat(app, "turDevTool.frameStats().paintedFrames"),
        stat(app, "turDevTool.frameStats().totals.flushUs"),
        stat(app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(app, "turDevTool.frameStats().totals.opsRecorded"),
    );

    for _ in 0..frames {
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }

    let post = (
        stat(app, "turDevTool.frameStats().flushes"),
        stat(app, "turDevTool.frameStats().paintedFrames"),
        stat(app, "turDevTool.frameStats().totals.flushUs"),
        stat(app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(app, "turDevTool.frameStats().totals.opsRecorded"),
    );

    let painted = post.1 - pre.1;
    assert!(painted > 0.0, "{label}: no frames painted");

    let flush_delta = post.0 - pre.0;
    let flush_us = post.2 - pre.2;
    let nodes = post.3 - pre.3;
    let ops = post.4 - pre.4;
    let per_frame_us = flush_us / painted;
    println!(
        "{label:<42} frames={:>4} flushes={:>4} us/frame={:>7.1} nodes/frame={:>7.1} ops/frame={:>7.1}",
        painted,
        flush_delta,
        per_frame_us,
        nodes / painted,
        ops / painted,
    );
}

/// 12×12 grid of colored Containers (nested Columns, ~156 nodes + wrappers),
/// one `Text` bound to a counter source. Each frame bumps the counter — the
/// minimal-change workload: one dirty node, full batch re-record.
pub fn static_tree(frames: usize) {
    let app = TurTestApp::new(400.0, 600.0).expect("app");
    app.eval_module_source(
        r##"
        import { mount, Column, Container, Text, source, Color, derive } from "tur:std";

        const tick$ = source(0);

        const rows = [];
        for (let i = 0; i < 12; i++) {
            const cells = [];
            for (let j = 0; j < 12; j++) {
                cells.push(Container()
                    .width(30).height(30)
                    .color(Color.hex("#208040"))
                    .build());
            }
            rows.push(Column().children(cells).build());
        }

        export function start({ store }) {
            Object.assign(globalThis, {
                __bump: () => { store.set(tick$, (store.get(tick$) | 0) + 1); },
            });
            mount(Column().children([
                ...rows,
                Text({ text: derive((ctx) => "t=" + ctx.get(tick$)) }).build(),
            ]).build());
        }
        "##,
    )
    .expect("load static_tree");
    drive_and_report(&app, "static-tree (156 cells, 1 text)", frames);
}

/// ScrollView over a 200-item Column of colored Containers; each frame
/// `jumpTo`es the scroll offset (the scroll workload).
pub fn scrolled_list(frames: usize) {
    let app = TurTestApp::new(400.0, 600.0).expect("app");
    app.eval_module_source(
        r##"
        import { mount, Column, Container, ScrollView, createScrollController, Color } from "tur:std";

        const controller = createScrollController({ initialOffset: 0 });
        const items = [];
        for (let i = 0; i < 200; i++) {
            items.push(Container()
                .height(40)
                .color(Color.hex("#204080"))
                .build());
        }
        mount(ScrollView()
            .controller(controller)
            .child(Column().children(items).build())
            .build());

        Object.assign(globalThis, {
            __scrollTo: (y) => { controller.jumpTo(y); },
        });
        "##,
    )
    .expect("load scrolled_list");

    // Quiesce before measuring, then scroll a little further each frame
    // (40px/frame — the full 8000px list over the 600px viewport never
    // finishes; that's fine, the workload is "steady scroll").
    app.wait_for_timeout(Duration::ZERO);
    let pre = (
        stat(&app, "turDevTool.frameStats().flushes"),
        stat(&app, "turDevTool.frameStats().paintedFrames"),
        stat(&app, "turDevTool.frameStats().totals.flushUs"),
        stat(&app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(&app, "turDevTool.frameStats().totals.opsRecorded"),
    );
    for i in 0..frames {
        let offset = ((i * 40) % 7600) as f64;
        app.eval_js(&format!("globalThis.__scrollTo({offset})"));
        app.pump();
    }
    let post = (
        stat(&app, "turDevTool.frameStats().flushes"),
        stat(&app, "turDevTool.frameStats().paintedFrames"),
        stat(&app, "turDevTool.frameStats().totals.flushUs"),
        stat(&app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(&app, "turDevTool.frameStats().totals.opsRecorded"),
    );
    let painted = post.1 - pre.1;
    assert!(painted > 0.0, "scrolled_list: no frames painted");
    println!(
        "{:<42} frames={:>4} flushes={:>4} us/frame={:>7.1} nodes/frame={:>7.1} ops/frame={:>7.1}",
        "scrolled-list (200×40px, jumpTo/frame)",
        painted,
        post.0 - pre.0,
        (post.2 - pre.2) / painted,
        (post.3 - pre.3) / painted,
        (post.4 - pre.4) / painted,
    );
}

/// An infinite opacity animation over 50 static colored Containers (the
/// continuous-frame workload — every frame differs).
pub fn animated_opacity(frames: usize) {
    let app = TurTestApp::new(400.0, 600.0).expect("app");
    app.eval_module_source(
        r##"
        import { mount, Column, Container, Opacity, Color, derive, mutate, source } from "tur:std";
        import { createAnimationController } from "tur:animation";

        const opacity$ = source(1);

        const ctrl = createAnimationController({
            duration: 1000,
            repeat: "infinite",
            onTick: mutate((ctx, t) => { ctx.set(opacity$, t); }),
        });
        ctrl.forward();

        const kids = [];
        for (let i = 0; i < 50; i++) {
            kids.push(Container().width(40).height(40).color(Color.hex("#3060c0")).build());
        }

        mount(Opacity().value(derive((ctx) => ctx.get(opacity$))).child(
            Column().children(kids).build(),
        ).build());
        "##,
    )
    .expect("load animated_opacity");

    // Steady animation: virtual clock advances, pump drives one frame each.
    app.wait_for_timeout(Duration::ZERO);
    let pre = (
        stat(&app, "turDevTool.frameStats().flushes"),
        stat(&app, "turDevTool.frameStats().paintedFrames"),
        stat(&app, "turDevTool.frameStats().totals.flushUs"),
        stat(&app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(&app, "turDevTool.frameStats().totals.opsRecorded"),
    );
    for _ in 0..frames {
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }
    let post = (
        stat(&app, "turDevTool.frameStats().flushes"),
        stat(&app, "turDevTool.frameStats().paintedFrames"),
        stat(&app, "turDevTool.frameStats().totals.flushUs"),
        stat(&app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(&app, "turDevTool.frameStats().totals.opsRecorded"),
    );
    let painted = post.1 - pre.1;
    assert!(painted > 0.0, "animated_opacity: no frames painted");
    println!(
        "{:<42} frames={:>4} flushes={:>4} us/frame={:>7.1} nodes/frame={:>7.1} ops/frame={:>7.1}",
        "animated-opacity (50 cells, infinite)",
        painted,
        post.0 - pre.0,
        (post.2 - pre.2) / painted,
        (post.3 - pre.3) / painted,
        (post.4 - pre.4) / painted,
    );
}

/// The 400-line spanned editor (the `rich_text_perf` fixture shape) inside a
/// ScrollView — the text-heavy workload. Frames scroll through the document.
pub fn long_editor(frames: usize) {
    let app = TurTestApp::new(400.0, 600.0).expect("app");
    app.eval_module_source(
        r##"
        import { mount, ScrollView, Input, createScrollController } from "tur:std";

        const spans = [];
        for (let i = 0; i < 400; i++) {
            spans.push({ content: "const value" + i + " = " + i + "; // line " + i + "\n" });
        }
        globalThis.__spans = spans;
        globalThis.__ctrl = new globalThis.TextEditingController();
        globalThis.__ctrl.setSpans(spans);
        const controller = createScrollController({ initialOffset: 0 });

        mount(ScrollView()
            .controller(controller)
            .queryKey(["scroll"])
            .child(Input()
                .controller(globalThis.__ctrl)
                .multiline(true)
                .fontFamily("monospace")
                .fontSize(14)
                .queryKey(["ed"])
                .build())
            .build());

        Object.assign(globalThis, {
            __scrollTo: (y) => { controller.jumpTo(y); },
        });
        "##,
    )
    .expect("load long_editor");

    app.wait_for_timeout(Duration::ZERO);
    let pre = (
        stat(&app, "turDevTool.frameStats().flushes"),
        stat(&app, "turDevTool.frameStats().paintedFrames"),
        stat(&app, "turDevTool.frameStats().totals.flushUs"),
        stat(&app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(&app, "turDevTool.frameStats().totals.opsRecorded"),
    );
    for i in 0..frames {
        let offset = ((i * 12) % 6000) as f64;
        app.eval_js(&format!("globalThis.__scrollTo({offset})"));
        app.pump();
    }
    let post = (
        stat(&app, "turDevTool.frameStats().flushes"),
        stat(&app, "turDevTool.frameStats().paintedFrames"),
        stat(&app, "turDevTool.frameStats().totals.flushUs"),
        stat(&app, "turDevTool.frameStats().totals.nodesWalked"),
        stat(&app, "turDevTool.frameStats().totals.opsRecorded"),
    );
    let painted = post.1 - pre.1;
    assert!(painted > 0.0, "long_editor: no frames painted");
    println!(
        "{:<42} frames={:>4} flushes={:>4} us/frame={:>7.1} nodes/frame={:>7.1} ops/frame={:>7.1}",
        "long-editor (400 lines, scroll)",
        painted,
        post.0 - pre.0,
        (post.2 - pre.2) / painted,
        (post.3 - pre.3) / painted,
        (post.4 - pre.4) / painted,
    );
}
