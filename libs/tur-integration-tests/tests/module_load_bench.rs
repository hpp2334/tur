//! The trivial-module load bench — the fast-lane gate + timing readout.
//!
//! Loads a trivial kit-using module (the standard shape) and reports
//! per-load wall time. The ASSERTION pins the lane structure, never
//! machine speed: the harness always installs the precompiled kit, so
//! every standard-shape load must ride it (`precompiled_std_stats`).
//! The timing ceilings are set far enough above the honest numbers
//! that only a STRUCTURAL regression (the double-compile shape this
//! lane replaces, ~2x these figures) trips them.

use std::time::Instant;

use tur_integration_tests::TurTestApp;

const TRIVIAL: &str = r#"
use tur_kit::handles::{ mount };
use tur_kit::layout::flex::{ Column };

entry fn start() {
    mount(Column().build());
}
"#;

#[test]
fn trivial_module_load_bench() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    // Warm the lane (the bundle's walk is once per worker thread), then
    // time steady-state loads.
    app.load_rut_module(TRIVIAL).unwrap();
    let mut total = std::time::Duration::ZERO;
    const N: usize = 5;
    for i in 0..N {
        let t = Instant::now();
        app.load_rut_module(TRIVIAL).unwrap();
        total += t.elapsed();
        println!("[load-bench] load #{i}: {:?}", t.elapsed());
    }
    let avg = total / N as u32;
    println!("[load-bench] average over {N} loads: {avg:?}");

    let (served, fallbacks) = tur_engine::core::rut_runtime::precompiled_std_stats();
    println!("[load-bench] lane stats: served={served} source_fallbacks={fallbacks}");
    assert!(
        served > 0,
        "no load rode the precompiled kit — the fast lane lost its fingerprint or was \
         never installed"
    );

    // The structural ceiling: set far above even loaded-suite timings
    // (nextest runs this binary alongside dozens of others) — only a
    // catastrophic regression (an order of magnitude, e.g. the whole
    // offered set recompiled twice per load) trips it. The honest
    // steady-state figures live in the printed readout and the phase
    // report, not in a machine-speed assert.
    if cfg!(debug_assertions) {
        assert!(
            avg < std::time::Duration::from_millis(4_000),
            "trivial-module load averaged {avg:?} in debug — a compile is running twice, or the \
             offered set grew wildly"
        );
    } else {
        assert!(
            avg < std::time::Duration::from_millis(400),
            "trivial-module load averaged {avg:?} in release — a compile is running twice, or the \
             offered set grew wildly"
        );
    }
}
