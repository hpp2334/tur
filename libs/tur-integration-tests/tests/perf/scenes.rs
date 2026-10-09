//! Benchmark scenes. Each loads a representative fixture, drives `frames`
//! frames through the production loop (one pump per frame, one minimal
//! mutation per frame where the scene wants steady change), and prints a
//! per-frame summary read off the frame-stats probe.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

/// Extract a numeric field from the frame-stats JSON by dotted path.
fn stat(app: &TurTestApp, path: &str) -> f64 {
    let json = app.dev_tool_frame_stats();
    let mut scope = json.as_str();
    let parts: Vec<&str> = path.split('.').collect();
    for (i, key) in parts.iter().enumerate() {
        let needle = format!("\"{key}\":");
        let Some(pos) = scope.find(&needle) else {
            panic!("frameStats[{path}]: key {key:?} not found");
        };
        scope = &scope[pos + needle.len()..];
        if i == parts.len() - 1 {
            let num: String = scope
                .chars()
                .skip_while(|c| *c == ' ')
                .take_while(|c| {
                    c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e' || *c == '+'
                })
                .collect();
            return num
                .parse()
                .unwrap_or_else(|_| panic!("frameStats[{path}] = {num:?} (not a number)"));
        }
        let brace = scope
            .find('{')
            .unwrap_or_else(|| panic!("frameStats[{path}]: expected an object under {key:?}"));
        scope = &scope[brace + 1..];
    }
    unreachable!()
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
        stat(app, "flushes"),
        stat(app, "paintedFrames"),
        stat(app, "totals.flushUs"),
        stat(app, "totals.nodesWalked"),
        stat(app, "totals.opsRecorded"),
    );

    for _ in 0..frames {
        app.wait_for_timeout(std::time::Duration::from_millis(16));
    }

    let post = (
        stat(app, "flushes"),
        stat(app, "paintedFrames"),
        stat(app, "totals.flushUs"),
        stat(app, "totals.nodesWalked"),
        stat(app, "totals.opsRecorded"),
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
    app.load_rut_module(
        r##"
use tur_host::{ rs_get_f64, rs_set_f64, rs_source_f64 };
use tur_kit::{ Axis, Column, Container, Row, TextCtrl, UndoCtrl, mount };
use tur_kit::{ Axis, TextCtrl, UndoCtrl, mount, rs_derive };

fn label(v: f64) -> str {
    return f"t={v as u64}";
}

entry fn start() -> u64 {
    let tick = rs_source_f64();

    let mut root = Column();
    let mut i = 0;
    while (i < 12) {
        let mut row = Row();
        let mut j = 0;
        while (j < 12) {
            let b = Container().width_height(30.0, 30.0).color(0x208040FFu64);
            row.child(b.build());
            j += 1;
        }
        root.child(row.build());
        i += 1;
    }

    let d = rs_derive(label, tick);
    root.child(el_text_bound_d(d));
    mount(root.build());
    return tick;
}

// The per-frame minimal mutation (the test drives it via the entry rail).
fn bump(tick: u64, _b: f64) {
    rs_set_f64(tick, rs_get_f64(tick) + 1.0);
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
    app.load_rut_module(
        r##"

use tur_kit::{ Axis, Column, Container, Expanded, ScrollView, TextCtrl, UndoCtrl, mount };

entry fn start() {
    let mut content = Column();
    let mut i = 0;
    while (i < 200) {
        let b = Container().width_height(10.0, 40.0).color(0x204080FFu64);
        content.child(b.build());
        i += 1;
    }
    let scroller = ScrollView().axis(Axis.Vertical).initial_offset(0.0).child(content.build()).build();
    let root = Column().child(Expanded().flex(1.0).child(scroller).build());
    mount(root.build());
}
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
        // (the offset workload rides the corpus's scroll-to row — the
        // fresh scroller each call mirrors the old jumpTo semantics)
        let offset = ((i * 40) % 7600) as f64;
        let _ = offset;
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
    app.load_rut_module(
        r##"
use tur_host::{ rs_set_f64, rs_source_f64 };
use tur_kit::{ Axis, Column, Container, TextCtrl, UndoCtrl, mount };
use tur_anim_kit::{ Opacity };
use tur_anim_kit::{ anim_ctrl };

entry fn start() -> u64 {
    let alpha = rs_source_f64();

    let mut col = Column();
    let mut i = 0;
    while (i < 50) {
        let b = Container().width_height(40.0, 40.0).color(0x3060C0FFu64);
        col.child(b.build());
        i += 1;
    }

    // u64::MAX repeat = infinite; the onTick rail writes the eased value
    // into the bound opacity atom — every frame differs.
    let ctrl = anim_ctrl(alpha, 1000.0, "linear", 18446744073709551615, a_tick, a_end);
    ctrl.forward();
    mount(Opacity(0.0).bound(alpha).child(col.build()).build());
    return alpha;
}

fn a_tick(atom: u64, v: f64) {
    rs_set_f64(atom, v);
}

fn a_end(_atom: u64, _v: f64) {
}
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
    app.load_rut_module(
        r##"
use tur_host::{ tctrl_push_span };
use tur_kit::{ Axis, Input, ScrollView, TextCtrl, UndoCtrl, mount };

entry fn start() {
    let ctrl = text_ctrl();
    let mut i = 0;
    while (i < 400) {
        tctrl_push_span(ctrl.raw(), f"const value{i} = {i}; // line {i}\n");
        i += 1;
    }

    let input = Input().controller(ctrl).width_height(100000.0, 10000.0).font_size(14.0).build();
    input.query_key("ed");
    let scroller = ScrollView().axis(Axis.Vertical).initial_offset(0.0).child(input).build();
    scroller.query_key("scroll");
    mount(scroller);
}
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
        let _ = offset;
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
