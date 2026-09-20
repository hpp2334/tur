//! Frame-stats probe (`core::app::frame_stats`) — the render-performance
//! instrumentation contract:
//!
//! - painted frames record worker-side timings + counters (`last`),
//! - host-side render-commit timings are **opt-in**
//!   (`turDevTool.setHostFrameTiming(true)`) and land in a separate
//!   `lastHost` slot (never merged into `last` — frames pipeline),
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

/// Read a numeric field off `turDevTool.frameStats()` via the test-only
/// `eval_js` seam (the dev-tool object is the engine's public face).
fn stat(app: &TurTestApp, field: &str) -> f64 {
    let raw = app.eval_js(&format!("String(turDevTool.frameStats().{field})"));
    raw.trim()
        .parse()
        .unwrap_or_else(|_| panic!("frameStats().{field} = {raw:?} (not a number)"))
}

/// Read a boolean-ish field as its JS display string (`"true"`/`"false"`/
/// `"null"` come back from `String(...)`).
fn stat_str(app: &TurTestApp, expression: &str) -> String {
    app.eval_js(&format!("String({expression})"))
        .trim()
        .to_string()
}

/// Mount a small static tree (a Container with a text-bearing child so the
/// record walk has real nodes + ops).
fn mount(app: &TurTestApp) {
    app.eval_module_source(
        r#"
        import { mount, Column, Container, Text } from "tur:std";
        mount(Column().children([
            Container().height(50).queryKey(["a"]).build(),
            Text({ text: "hello" }).build(),
        ]).build());
        "#,
    )
    .expect("mount");
    app.wait_for_timeout(std::time::Duration::ZERO);
}

/// A painted frame records worker-side timings + counters; `last.frame_id`
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

    // last.frame_id is a valid flush epoch (monotonic ≥ 1).
    let last_frame = stat(&app, "last.frame");
    assert!(last_frame >= 1.0, "last.frame_id = {last_frame}");

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
    assert_eq!(
        stat_str(&app, "turDevTool.frameStats().lastHost"),
        "null",
        "lastHost must be null while disabled"
    );

    // Enable + force a fresh painted frame (loading a second module mounts
    // a new root → paint).
    app.eval_js("turDevTool.setHostFrameTiming(true)");
    app.eval_module_source(
        r#"
        import { mount, Container, createColor } from "tur:std";
        mount(Container().height(10).color(createColor(255, 0, 0, 255)).build());
        "#,
    )
    .expect("remount");
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert_eq!(
        stat_str(&app, "turDevTool.frameStats().hostTimingEnabled"),
        "true"
    );
    let host_present = stat_str(&app, "turDevTool.frameStats().lastHost !== null");
    assert_eq!(host_present, "true", "expected a host frame timing");
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
    let before_painted = stat(&app, "paintedFrames");
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
