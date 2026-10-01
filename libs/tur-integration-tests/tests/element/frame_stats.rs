//! Frame-stats probe (`core::app::frame_stats`) — the render-performance
//! instrumentation contract:
//!
//! - painted frames record worker-side timings + counters (`last`),
//! - host-side render-commit timings are **opt-in**
//!   (`setHostFrameTiming(true)`) and land in a separate `lastHost` slot
//!   (never merged into `last` — frames pipeline),
//! - the toggle is mirrored on both sides (`hostTimingEnabled`),
//! - flushes count every flush; painted frames count the subset that
//!   produced a batch.
//!
//! Empirical note for the perf audit: a quiesced `pump()` still repaints in
//! the current engine (every Wake flush reports `needs_paint` and ships a
//! batch — an empty one for colorless/root-less trees). That is exactly the
//! redundant work the Phase-1 frame-dedup (batch fingerprint) is aimed at,
//! so these tests pin the counter bookkeeping, not "idle stays quiet".

use tur_integration_tests::TurTestApp;

/// Extract a numeric field from the frame-stats JSON by dotted path
/// (`"paintedFrames"`, `"last.frame"`, `"lastHost.applyUs"`).
fn stat(app: &TurTestApp, path: &str) -> f64 {
    let json = app.dev_tool_frame_stats();
    let mut scope = json.as_str();
    let parts: Vec<&str> = path.split('.').collect();
    for (i, key) in parts.iter().enumerate() {
        let needle = format!("\"{key}\":");
        let Some(pos) = scope.find(&needle) else {
            panic!("frameStats[{path}]: key {key:?} not found in {scope:.120}");
        };
        scope = &scope[pos + needle.len()..];
        let last = i == parts.len() - 1;
        if last {
            // The value starts here: a number (or `null`).
            let num: String = scope
                .chars()
                .skip_while(|c| *c == ' ')
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e' || *c == '+')
                .collect();
            return num
                .parse()
                .unwrap_or_else(|_| panic!("frameStats[{path}] = {num:?} (not a number)"));
        }
        // Descend into the object: find its opening brace.
        let brace = scope.find('{').unwrap_or_else(|| {
            panic!("frameStats[{path}]: expected an object under {key:?}");
        });
        scope = &scope[brace + 1..];
    }
    unreachable!()
}

/// Whether a sub-object slot is `null` (absent) or populated.
fn stat_present(app: &TurTestApp, key: &str) -> bool {
    let json = app.dev_tool_frame_stats();
    let needle = format!("\"{key}\":");
    let Some(pos) = json.find(&needle) else {
        return false;
    };
    !json[pos + needle.len()..].starts_with("null")
}

/// Mount a small static tree (a box with a text-bearing child so the
/// record walk has real nodes + ops).
fn mount(app: &TurTestApp) {
    app.load_rut_module(
        r#"
use tur::{ mount };
use tur_kit::{ Column, Container, Text };


entry fn start() {
    let mut col = Column.new();
    let mut b = Container.new();
    b.width_height(100.0, 50.0);
    col.child(b.build());
    col.child(Text.new().text("hello").build());
    mount(col.build());
}
"#,
    )
    .expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);
}

/// A painted frame records worker-side timings + counters; `last.frame`
/// is a real flush epoch and the batch-size estimate is populated.
#[test]
fn painted_frame_records_worker_counters() {
    let app = TurTestApp::new(300.0, 300.0).expect("app");
    mount(&app);

    let painted = stat(&app, "paintedFrames");
    assert!(painted >= 1.0, "expected ≥1 painted frame, got {painted}");
    let flushes = stat(&app, "flushes");
    assert!(
        flushes >= painted,
        "flushes ({flushes}) must count painted frames too"
    );

    // last.frame is a valid flush epoch (monotonic ≥ 1).
    let last_frame = stat(&app, "last.frame");
    assert!(last_frame >= 1.0, "last.frame = {last_frame}");

    // The record walk entered at least the root + children.
    let nodes = stat(&app, "last.nodesWalked");
    assert!(nodes >= 2.0, "nodesWalked = {nodes}");

    // Batch estimate: commands + ops both populated.
    let commands = stat(&app, "last.commands");
    assert!(commands >= 1.0, "commands = {commands}");
    let ops = stat(&app, "last.opsRecorded");
    assert!(ops >= 1.0, "opsRecorded = {ops}");
    let bytes = stat(&app, "last.batchBytes");
    assert!(bytes > 0.0, "batchBytes = {bytes}");
}

/// Host frame timing is opt-in: `lastHost` is `null` until
/// `setHostFrameTiming(true)`, then the next painted frame carries host
/// render-commit timings attributed to a real flush epoch.
#[test]
fn host_frame_timing_is_opt_in() {
    let app = TurTestApp::new(300.0, 300.0).expect("app");
    mount(&app);

    // Off by default — zero host timings collected.
    assert!(
        !stat_present(&app, "lastHost"),
        "lastHost must be absent while disabled"
    );

    // Enable + force a fresh painted frame (loading a second module mounts
    // a new root → paint).
    app.set_host_frame_timing(true);
    app.load_rut_module(
        r#"use tur::{ mount };
use tur_kit::{ Container };

use tur_kit::{ Container };


entry fn start() {
    let mut b = Container.new();
    b.width_height(100.0, 10.0);
    b.color(0xFF0000FFu64);
    mount(b.build());
}
"#,
    )
    .expect("remount");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let json = app.dev_tool_frame_stats();
    assert!(
        json.contains("\"hostTimingEnabled\":true"),
        "the toggle is mirrored into the snapshot: {json}"
    );
    assert!(stat_present(&app, "lastHost"), "expected a host frame timing");
    let host_frame = stat(&app, "lastHost.frame");
    assert!(host_frame >= 1.0, "lastHost.frame = {host_frame}");
    // Timings are non-negative (0µs possible for a sub-µs noop render).
    assert!(stat(&app, "lastHost.applyUs") >= 0.0);
    assert!(stat(&app, "lastHost.presentUs") >= 0.0);
}

/// Every flush bumps `flushes`; only painted flushes advance `last.frame`.
/// (Empirically a quiesced Wake flush still paints in the current engine —
/// see the idle-repaint note in the module docs — so this pins the counter
/// bookkeeping without assuming idle pumps stay unpainted.)
#[test]
fn flushes_count_all_painted_or_not() {
    let mut app = TurTestApp::new(400.0, 600.0).expect("app");
    app.load_bundle("clickable-text").expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);

    let before_flushes = stat(&app, "flushes");
    let before_last_frame = stat(&app, "last.frame");

    for _ in 0..3 {
        app.pump();
    }
    let after_flushes = stat(&app, "flushes");
    let after_painted = stat(&app, "paintedFrames");
    assert!(
        after_flushes > before_flushes,
        "pumps must bump flushes ({before_flushes} → {after_flushes})"
    );
    // painted ⊆ flushes, always.
    assert!(
        after_painted <= after_flushes,
        "paintedFrames ({after_painted}) must never exceed flushes ({after_flushes})"
    );
    // `last.frame` only ever moves forward, and only on painted flushes.
    let after_last_frame = stat(&app, "last.frame");
    assert!(
        after_last_frame >= before_last_frame,
        "last.frame went backwards ({before_last_frame} → {after_last_frame})"
    );
}
