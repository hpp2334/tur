//! The `Switch` control-flow element over the `tur` host pkg's switch
//! builder rows (`el_switch` + `switch_case` / `switch_fallback`): initial
//! branch, keyed swap, fallback, same-key no-rebuild, and derived-value
//! swaps through the subscriber graph.

use tur_integration_tests::TurTestApp;

/// A switch bound to a str atom with two cases + a fallback; `set_key`
/// mutates the atom (the test's flip rail).
const RUNTIME: &str = r#"
use tur::{ ctx_bridge, mount };
use tur_kit::{ SourceF64,  MutationCtx, Readable, Switch, Text, source_str };


entry fn start() -> u64 {
    let key: Readable<str> = source_str("a");

    let mut sw = Switch().value(key);
    sw.cases("a", Text().text("AAA").query_key("case_a").build());
    sw.cases("b", Text().text("BBB").query_key("case_b").build());
    sw.fallback(Text().text("FALL").query_key("case_fallback").build());
    mount(sw.build());
    return key.atom_id();
}

entry fn set_key(atom: u64, _b: f64) {
    let key = SourceStr.of(ctx_bridge(), atom, false);
    MutationCtx.over(ctx_bridge()).set<str>(key, "b");
}

entry fn set_key_raw(atom: u64, _b: f64) {
    let key = SourceStr.of(ctx_bridge(), atom, false);
    MutationCtx.over(ctx_bridge()).set<str>(key, "zzz");
}

entry fn reemit(atom: u64, _b: f64) {
    let key = SourceStr.of(ctx_bridge(), atom, false);
    MutationCtx.over(ctx_bridge()).set<str>(key, "a");
}
"#;

fn mount_switch() -> (TurTestApp, u64) {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_rut_module(RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let key = app.rut_start_answer();
    (app, key)
}

#[test]
fn switch_mounts_initial_branch() {
    let (app, _key) = mount_switch();

    // The "a" branch should be mounted; "b" and fallback should not.
    assert!(
        app.query_element(&["case_a"]).is_some(),
        "case_a should be mounted"
    );
    assert!(
        app.query_element(&["case_b"]).is_none(),
        "case_b should NOT be mounted"
    );
    assert!(
        app.query_element(&["case_fallback"]).is_none(),
        "fallback should NOT be mounted",
    );
}

#[test]
fn switch_swaps_branch_on_value_change() {
    let (mut app, key) = mount_switch();

    assert!(app.query_element(&["case_a"]).is_some());

    // Flip the value atom to "b".
    app.call_rut_entry("set_key", key, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.query_element(&["case_a"]).is_none(),
        "case_a should be torn down"
    );
    assert!(
        app.query_element(&["case_b"]).is_some(),
        "case_b should now be mounted",
    );
}

#[test]
fn switch_uses_fallback_when_no_case_matches() {
    let (mut app, key) = mount_switch();

    // Value with no matching case → fallback branch.
    app.call_rut_entry("set_key_raw", key, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(app.query_element(&["case_a"]).is_none());
    assert!(app.query_element(&["case_b"]).is_none());
    assert!(
        app.query_element(&["case_fallback"]).is_some(),
        "fallback should be mounted when no case matches",
    );
}

#[test]
fn switch_no_rebuild_when_value_re_emits_same_key() {
    let (mut app, key) = mount_switch();

    let a_id = app.query_element(&["case_a"]).unwrap();
    // Re-set the same key — the mounted node identity should be unchanged.
    app.call_rut_entry("reemit", key, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let a_id_after = app.query_element(&["case_a"]).unwrap();
    assert_eq!(a_id, a_id_after, "same key must not trigger a rebuild");
}

/// A switch bound to a DERIVED atom (`rs_derive` over the source): the swap
/// rides the subscriber graph when the dep flips. `rs_derive` crosses its
/// dep as f64 (`entry fn d(dep: f64) -> str`), so the source is a numeric
/// atom the derive maps onto the string keys.
const DERIVED_RUNTIME: &str = r#"
use tur::{ ctx_bridge, mount, rs_set_f64, rs_source_f64 };
use tur_kit::{ MutationCtx, Readable, Switch, Text };
use tur_kit::{ rs_derive };

fn d(v: f64) -> str {
    if (v == 1.0) {
        return "b";
    }
    if (v == 2.0) {
        return "zzz";
    }
    return "a";
}

entry fn start() -> u64 {
    let key = rs_source_f64();
    rs_set_f64(key, 0.0);
    let derived = rs_derive(d, key);

    // The unified value prop over the derived handle (the kind rides the
    // handle: a derived binds through the derive rail).
    let mut sw = Switch().value(SourceStr.of(ctx_bridge(), derived, false));
    sw.cases("a", Text().text("AAA").query_key("d_case_a").build());
    sw.cases("b", Text().text("BBB").query_key("d_case_b").build());
    sw.fallback(Text().text("FALL").query_key("d_case_fallback").build());
    mount(sw.build());
    return key;
}

entry fn set_key(atom: u64, _b: f64) {
    let key = SourceF64.of(ctx_bridge(), atom, false);
    MutationCtx.over(ctx_bridge()).set<f64>(key, 1.0);
}
"#;

#[test]
fn switch_swaps_branch_on_derived_value_change() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_rut_module(DERIVED_RUNTIME).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);
    let key = app.rut_start_answer();

    assert!(
        app.query_element(&["d_case_a"]).is_some(),
        "d_case_a should be mounted initially"
    );

    // Flip the source atom — the derived goes stale and the Switch swaps
    // via the subscriber graph (not a full-scan try_rebuild).
    app.call_rut_entry("set_key", key, 0.0).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    assert!(
        app.query_element(&["d_case_a"]).is_none(),
        "d_case_a should be torn down"
    );
    assert!(
        app.query_element(&["d_case_b"]).is_some(),
        "d_case_b should now be mounted"
    );
}
