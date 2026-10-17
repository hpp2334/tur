//! The atom-only bind lanes' atom-0 guard. `sv(...)` structurally
//! satisfies `Readable<T>` (`atom_id() == 0` — the reactive counter
//! starts at 1), so a static value passed to an atom-only row used to
//! silently bind atom 0 and read garbage forever. The rows trap on it
//! instead: the raw-row probe below loads a module whose `start` calls a
//! guarded row with 0 and asserts the LOUD load failure (the trap names
//! the hazard and the row), plus the positive control — the same row
//! with a real atom boots fine.

use tur_integration_tests::TurTestApp;

/// Raw-row probe: `scroll::lazy_count(spec, 0)` — the count lane is
/// atom-only, so atom 0 is the `sv()` slip and `start` traps at boot
/// (the corpus boots eagerly; the load fails with the guard message).
const SV_ON_COUNT_LANE: &str = r#"
use tur_host::scroll::{ lazy_count, lazy_list_new };

entry fn start() -> u64 {
    let spec = lazy_list_new();
    lazy_count(spec, 0);
    return 0;
}
"#;

/// The positive control: the same lane with a REAL atom boots and the
/// module's answer is the minted atom id (the counter starts at 1 — 0
/// stays the static discriminator).
const REAL_ATOM_ON_COUNT_LANE: &str = r#"
use tur_host::scroll::{ lazy_count, lazy_list_new };
use tur_host::store::{ rs_source_f64 };

entry fn start() -> u64 {
    let atom = rs_source_f64();
    let spec = lazy_list_new();
    lazy_count(spec, atom);
    return atom;
}
"#;

#[test]
fn atom_only_lane_rejects_sv_loudly() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    let err = app.load_rut_module(SV_ON_COUNT_LANE).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("static value on an atom-only lane"),
        "the guard message must name the hazard: {msg}"
    );
    assert!(msg.contains("lazy_count"), "the guard names the row: {msg}");
}

#[test]
fn atom_only_lane_accepts_a_real_atom() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(REAL_ATOM_ON_COUNT_LANE).unwrap();
    assert_ne!(
        app.rut_start_answer(),
        0,
        "the probe answer is a real minted atom (the counter starts at 1 — 0 stays the \
         static discriminator, so a bound lane never sees it)"
    );
}
