//! Phase-6 lazy branch builders: `Condition().then_build(cb)` /
//! `.else_build(cb)` and `Switch().case_build(key, cb)` take FN VALUES —
//! the kit seals the fn into an opaque box, the row stores it, and the
//! engine invokes it at ACTIVATION through the `__tur_cb_build0` dispatch
//! entry — re-invoking it at every RE-activation (the boa callback
//! semantic: the branch reads LIVE state each time it mounts, never a
//! stale pre-built subtree).
//!
//! Pinned here:
//! 1. the inactive branch's rows never run at boot (no marker, no side
//!    effect);
//! 2. activation invokes the builder (marker mounts, side effect ran);
//! 3. re-activation re-invokes it — a value changed while the branch was
//!    dormant shows up in the rebuilt subtree (fresh read of live state).

use std::time::Duration;

use tur_integration_tests::TurTestApp;

const COND_RUT: &str = r#"
use tur::{ mount, rs_get_f64, rs_set_bool, rs_set_f64, rs_source_bool, rs_source_f64, stf_put, stf_take };
use tur_kit::{ Condition, Column, Text };

let K_OPEN: u64 = 1;
let K_SEED: u64 = 2;

entry fn start() {
    let open = rs_source_bool(false);
    let seed = rs_source_f64();
    rs_set_f64(seed, 1.0);
    let runs = rs_source_f64();
    let root = Column().query_key("cb/root").child(
        Condition(open)
            .then_build(fn () -> View {
                rs_set_f64(runs, rs_get_f64(runs) + 1.0);
                return Text().text(f"modal {rs_get_f64(seed) as u64}").query_key("cb/then").build();
            })
            .else_build(fn () -> View {
                return Text().text("closed").query_key("cb/else").build();
            })
            .build(),
    ).build();
    stf_put(K_OPEN, open as f64);
    stf_put(K_SEED, seed as f64);
    mount(root);
}

entry fn probe_open(a: u64, _b: f64) {
    let open = stf_take(K_OPEN) as u64;
    rs_set_bool(open, a != 0);
    stf_put(K_OPEN, open as f64);
}

entry fn probe_seed(_a: u64, b: f64) {
    let seed = stf_take(K_SEED) as u64;
    rs_set_f64(seed, b);
    stf_put(K_SEED, seed as f64);
}
"#;

#[test]
fn inactive_branch_rows_never_run_and_activation_invokes() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(COND_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Boot with `open = false`: the ELSE branch is mounted and the THEN
    // branch's rows never ran — its marker is absent (a pre-built branch
    // would have authored "modal 1" at boot time).
    assert_eq!(app.query_text(&["cb", "else"]).as_deref(), Some("closed"));
    assert_eq!(
        app.query_text(&["cb", "then"]),
        None,
        "inactive branch rows must not run"
    );

    // Activation: the then builder runs NOW — reads live state, mounts
    // the fresh marker, and the else branch unmounts.
    app.call_rut_entry("probe_open", 1, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["cb", "then"]).as_deref(), Some("modal 1"));
    assert_eq!(app.query_text(&["cb", "else"]), None);
}

#[test]
fn reactivation_reinvokes_the_builder_with_live_state() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(COND_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Open → close → change the seed while dormant → open again. The
    // rebuilt branch must read the NEW seed ("modal 2"): the builder is
    // re-invoked at re-activation, not cloned from a boot-time pre-build.
    app.call_rut_entry("probe_open", 1, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["cb", "then"]).as_deref(), Some("modal 1"));

    app.call_rut_entry("probe_open", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["cb", "else"]).as_deref(), Some("closed"));

    app.call_rut_entry("probe_seed", 0, 2.0).unwrap();
    app.call_rut_entry("probe_open", 1, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        app.query_text(&["cb", "then"]).as_deref(),
        Some("modal 2"),
        "re-activation must re-invoke the builder (fresh read of live state)"
    );
}

// ---------------------------------------------------------------------------
// Switch().case_build(key, cb) — the lazy case twin.
// ---------------------------------------------------------------------------

const SWITCH_RUT: &str = r#"
use tur::{ mount, rs_set_str, rs_source_str, stf_put, stf_take };
use tur_kit::{ Column, Switch, Text };

let K_TAB: u64 = 4;

entry fn start() {
    let tab = rs_source_str("b");
    let label = rs_source_str("idle");
    let v = Switch().value_source(tab)
        .case_build("a", fn () -> View {
            rs_set_str(label, "built-a");
            return Text().text("A").query_key("sw/a").build();
        })
        .fallback(Text().text("fallback").query_key("sw/fb").build())
        .build();
    let root = Column().query_key("sw/root")
        .child(v)
        .child(Text().text_bound(label).query_key("sw/label").build())
        .build();
    stf_put(K_TAB, tab as f64);
    mount(root);
}

entry fn probe_tab(a: u64, _b: f64) {
    let tab = stf_take(K_TAB) as u64;
    if (a != 0) {
        rs_set_str(tab, "a");
    } else {
        rs_set_str(tab, "b");
    }
    stf_put(K_TAB, tab as f64);
}
"#;

#[test]
fn switch_case_build_invokes_only_when_the_key_activates() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(SWITCH_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    // Boot with tab "b": no case matches, the fallback mounts, and the
    // "a" case's rows never ran (its label side effect stayed "idle").
    assert_eq!(app.query_text(&["sw", "fb"]).as_deref(), Some("fallback"));
    assert_eq!(
        app.query_text(&["sw", "a"]),
        None,
        "inactive case rows must not run"
    );
    assert_eq!(app.query_text(&["sw", "label"]).as_deref(), Some("idle"));

    // Activate "a": the case builder runs now — marker + side effect.
    app.call_rut_entry("probe_tab", 1, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["sw", "a"]).as_deref(), Some("A"));
    assert_eq!(app.query_text(&["sw", "label"]).as_deref(), Some("built-a"));
    assert_eq!(app.query_text(&["sw", "fb"]), None);

    // Back to "b": the fallback remounts.
    app.call_rut_entry("probe_tab", 0, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["sw", "fb"]).as_deref(), Some("fallback"));
    assert_eq!(app.query_text(&["sw", "a"]), None);
}
