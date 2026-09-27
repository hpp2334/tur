//! Render-performance benchmarks — the Phase-0 measurement harness.
//!
//! These are `#[ignore]`d by default (nextest/CI never runs them); run
//! explicitly in release mode:
//!
//! ```sh
//! cargo test -p tur-integration-tests --test perf --release -- --ignored --nocapture
//! ```
//!
//! Each bench loads a representative scene, drives `N` frames through the
//! production loop (`TurTestApp::pump`, identical to what wasm/Android run),
//! and reads the instance's frame-stats probe (`turDevTool.frameStats()`)
//! for per-frame worker-side cost. Prints a summary line; asserts nothing
//! except liveness (frames were painted) — numbers are for humans.

mod scenes;

#[test]
#[ignore]
fn bench_static_tree_minimal_change() {
    scenes::static_tree(120);
}

#[test]
#[ignore]
fn bench_scrolled_list() {
    scenes::scrolled_list(120);
}

#[test]
#[ignore]
fn bench_animated_opacity_over_static_subtree() {
    scenes::animated_opacity(120);
}

#[test]
#[ignore]
fn bench_long_spanned_editor() {
    scenes::long_editor(60);
}
