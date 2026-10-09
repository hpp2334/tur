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
use tur_kit::{ Column, Condition, MutationCtx, Readable, Source, Text, entry_ctx, mount, source };

struct AppContext {
    open: Source<bool>,
    seed: Source<f64>,
}

entry fn start() -> opaque {
    // CONCRETE annotations — the fields cross as Source (the one
    // Writable; an interface-typed value does not fill the field).
    let open: Source<bool> = source<bool>(false);
    let seed: Source<f64> = source<f64>(1.0);
    let runs: Readable<f64> = source<f64>(0.0);
    let root = Column().query_key("cb/root").child(
        Condition(open)
            .then_build(fn () -> View {
                // The builder runs at ACTIVATION — a flush-time face call;
                // its ctx is the entry rail's.
                let write = entry_ctx();
                write.set<f64>(runs, write.get<f64>(runs) + 1.0);
                return Text().text(f"modal {write.get<f64>(seed) as u64}").query_key("cb/then").build();
            })
            .else_build(fn () -> View {
                return Text().text("closed").query_key("cb/else").build();
            })
            .build(),
    ).build();
    mount(root);
    return opaque(AppContext { open: open, seed: seed });
}

// The probe seam's downcast (one nil-guard, shared).
fn cond_cx(cx: opaque) -> AppContext {
    let c = opaque.downcast<AppContext>(cx);
    if (c == nil) {
        panic("cond fixture: cx is not an AppContext");
    }
    return c;
}

entry fn probe_open(cx: opaque, v: f64) {
    entry_ctx().set<bool>(cond_cx(cx).open, v != 0.0);
}

entry fn probe_seed(cx: opaque, b: f64) {
    entry_ctx().set<f64>(cond_cx(cx).seed, b);
}
"#;

#[test]
fn inactive_branch_rows_never_run_and_activation_invokes() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(COND_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    // The eager context-crossing shape: start answers the AppContext and
    // the token IS the start answer.
    let cx = app.rut_start_answer();
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
    app.call_rut_entry_cx_f64("probe_open", cx, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["cb", "then"]).as_deref(), Some("modal 1"));
    assert_eq!(app.query_text(&["cb", "else"]), None);
}

#[test]
fn reactivation_reinvokes_the_builder_with_live_state() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(COND_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let cx = app.rut_start_answer();
    app.wait_for_timeout(Duration::ZERO);

    // Open → close → change the seed while dormant → open again. The
    // rebuilt branch must read the NEW seed ("modal 2"): the builder is
    // re-invoked at re-activation, not cloned from a boot-time pre-build.
    app.call_rut_entry_cx_f64("probe_open", cx, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["cb", "then"]).as_deref(), Some("modal 1"));

    app.call_rut_entry_cx_f64("probe_open", cx, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["cb", "else"]).as_deref(), Some("closed"));

    app.call_rut_entry_cx_f64("probe_seed", cx, 2.0).unwrap();
    app.call_rut_entry_cx_f64("probe_open", cx, 1.0).unwrap();
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
use tur_kit::{ Column, MutationCtx, Source, Switch, Text, entry_ctx, mount, source };

struct AppContext {
    tab: Source<str>,
}

entry fn start() -> opaque {
    let tab: Source<str> = source<str>("b");
    let label: Source<str> = source<str>("idle");
    let v = Switch().value(tab)
        .case_build("a", fn () -> View {
            // The builder runs at ACTIVATION — its ctx is the entry
            // rail's.
            let write = entry_ctx();
            write.set<str>(label, "built-a");
            return Text().text("A").query_key("sw/a").build();
        })
        .fallback(Text().text("fallback").query_key("sw/fb").build())
        .build();
    let root = Column().query_key("sw/root")
        .child(v)
        .child(Text().text_bound(label).query_key("sw/label").build())
        .build();
    mount(root);
    return opaque(AppContext { tab: tab });
}

entry fn probe_tab(cx: opaque, flag: f64) {
    let c = opaque.downcast<AppContext>(cx);
    if (c == nil) {
        panic("switch fixture: cx is not an AppContext");
    }
    let write = entry_ctx();
    if (flag != 0.0) {
        write.set<str>(c.tab, "a");
    } else {
        write.set<str>(c.tab, "b");
    }
}
"#;

#[test]
fn switch_case_build_invokes_only_when_the_key_activates() {
    let mut app = TurTestApp::new(400.0, 300.0).unwrap();
    app.load_rut_module(SWITCH_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    let tab_atom = app.rut_start_answer();

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
    app.call_rut_entry_cx_f64("probe_tab", tab_atom, 1.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["sw", "a"]).as_deref(), Some("A"));
    assert_eq!(app.query_text(&["sw", "label"]).as_deref(), Some("built-a"));
    assert_eq!(app.query_text(&["sw", "fb"]), None);

    // Back to "b": the fallback remounts.
    app.call_rut_entry_cx_f64("probe_tab", tab_atom, 0.0).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(app.query_text(&["sw", "fb"]).as_deref(), Some("fallback"));
    assert_eq!(app.query_text(&["sw", "a"]), None);
}
