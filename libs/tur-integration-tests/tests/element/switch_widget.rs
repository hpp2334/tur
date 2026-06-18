use tur_integration_tests::TurTestApp;

const RUNTIME: &str = r#"
const T = globalThis.__tur;
const ctx = T.__ctx;
globalThis.__key = T.source(ctx, "a");
const root = T.Switch(ctx, {
    value: globalThis.__key,
    cases: [
        { key: "a", child: T.Text(ctx, { text: "AAA", queryKey: ["case_a"] }) },
        { key: "b", child: T.Text(ctx, { text: "BBB", queryKey: ["case_b"] }) },
    ],
    fallback: T.Text(ctx, { text: "FALL", queryKey: ["case_fallback"] }),
});
T.render(ctx, root);
"#;

#[test]
fn switch_mounts_initial_branch() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();

    // The "a" branch should be mounted; "b" and fallback should not.
    assert!(app.query_element(&["case_a"]).is_some(), "case_a should be mounted");
    assert!(app.query_element(&["case_b"]).is_none(), "case_b should NOT be mounted");
    assert!(
        app.query_element(&["case_fallback"]).is_none(),
        "fallback should NOT be mounted",
    );
}

#[test]
fn switch_swaps_branch_on_value_change() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();

    assert!(app.query_element(&["case_a"]).is_some());

    // Flip the value atom to "b".
    app.eval_js(
        r#"globalThis.__tur.set(globalThis.__tur.__ctx, globalThis.__key, "b");"#,
    );
    app.render();

    assert!(app.query_element(&["case_a"]).is_none(), "case_a should be torn down");
    assert!(
        app.query_element(&["case_b"]).is_some(),
        "case_b should now be mounted",
    );
}

#[test]
fn switch_uses_fallback_when_no_case_matches() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();

    // Value with no matching case → fallback branch.
    app.eval_js(
        r#"globalThis.__tur.set(globalThis.__tur.__ctx, globalThis.__key, "zzz");"#,
    );
    app.render();

    assert!(app.query_element(&["case_a"]).is_none());
    assert!(app.query_element(&["case_b"]).is_none());
    assert!(
        app.query_element(&["case_fallback"]).is_some(),
        "fallback should be mounted when no case matches",
    );
}

#[test]
fn switch_no_rebuild_when_value_re_emits_same_key() {
    let mut app = TurTestApp::new(200.0, 100.0).unwrap();
    app.load_bundle_source(RUNTIME).unwrap();
    app.render();

    let a_id = app.query_element(&["case_a"]).unwrap();
    // Re-set the same key — the mounted node identity should be unchanged.
    app.eval_js(
        r#"globalThis.__tur.set(globalThis.__tur.__ctx, globalThis.__key, "a");"#,
    );
    app.render();
    let a_id_after = app.query_element(&["case_a"]).unwrap();
    assert_eq!(a_id, a_id_after, "same key must not trigger a rebuild");
}
